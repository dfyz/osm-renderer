use crate::coords::Coords;
use crate::tile;
use anyhow::{Context, Result};
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use memmap2::{Mmap, MmapOptions};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Cursor;
use std::mem;
use std::ops::Deref;
use std::slice;
use std::str;

pub trait OsmEntity<'a> {
    fn global_id(&self) -> u64;
    fn tags(&self) -> Tags<'a>;
}

pub struct OsmEntities<'a> {
    pub nodes: Vec<Node<'a>>,
    pub ways: Vec<Way<'a>>,
    pub multipolygons: Vec<Multipolygon<'a>>,
}

#[derive(Default)]
pub(super) struct OsmEntityIds {
    pub(super) nodes: Vec<u32>,
    pub(super) ways: Vec<u32>,
    pub(super) multipolygons: Vec<u32>,
}

pub trait OsmArea {
    fn is_closed(&self) -> bool;
}

pub struct GeodataReader<'a> {
    storages: ObjectStorages<'a>,
    _mmap: Mmap,
}

impl<'a> GeodataReader<'a> {
    pub fn load(file_name: &str) -> Result<GeodataReader<'a>> {
        let input_file = File::open(file_name).context(format!("Failed to open {} for memory mapping", file_name))?;
        let mmap = unsafe {
            MmapOptions::new()
                .map(&input_file)
                .context(format!("Failed to map {} to memory", file_name))?
        };

        let raw_mmap_bytes = mmap.deref() as *const [u8];
        // `raw_mmap_bytes` points to bytes that are destroyed when `mmap` is dropped.
        // The bytes are only ever accessed from `storages`, which is bundled together with `mmap`
        // in `GeodataReader`. Therefore, `mmap` is still not dropped whenever we access the bytes.
        let storages = ObjectStorages::from_bytes(unsafe { &*raw_mmap_bytes });
        Ok(GeodataReader { storages, _mmap: mmap })
    }

    pub fn get_entities_in_tile_with_neighbors(
        &'a self,
        t: &tile::Tile,
        osm_ids: &Option<HashSet<u64>>,
    ) -> OsmEntities {
        let mut entity_ids = OsmEntityIds::default();

        let deltas = [-1, 0, 1];
        for dx in &deltas {
            for dy in &deltas {
                let adjacent_tile = tile::Tile {
                    x: (t.x as i32 + dx) as u32,
                    y: (t.y as i32 + dy) as u32,
                    zoom: t.zoom,
                };
                self.get_entities_in_tile(&adjacent_tile, &mut entity_ids);
            }
        }

        let uniq = |ids: &mut Vec<u32>| {
            ids.sort_unstable();
            ids.dedup();
        };

        uniq(&mut entity_ids.nodes);
        uniq(&mut entity_ids.ways);
        uniq(&mut entity_ids.multipolygons);

        let nodes = entity_ids.nodes.iter().map(|id| self.get_node(*id as usize));
        let ways = entity_ids.ways.iter().map(|id| self.get_way(*id as usize));
        let multipolygons = entity_ids.multipolygons.iter().filter_map(|id| {
            let mp = self.get_multipolygon(*id as usize);
            if mp.polygon_count() > 0 {
                Some(mp)
            } else {
                None
            }
        });

        OsmEntities {
            nodes: filter_entities_by_ids(nodes, osm_ids),
            ways: filter_entities_by_ids(ways, osm_ids),
            multipolygons: filter_entities_by_ids(multipolygons, osm_ids),
        }
    }

    pub(super) fn get_entities_in_tile(&'a self, t: &tile::Tile, entity_ids: &mut OsmEntityIds) {
        let mut bounds = tile::tile_to_max_zoom_tile_range(t);
        let mut start_from_index = 0;

        let tile_count = self.tile_count();
        while start_from_index < tile_count {
            match self.next_good_tile(&mut bounds, start_from_index) {
                None => break,
                Some(mut current_index) => {
                    let (mut tile_x, mut tile_y) = self.tile_xy(current_index);
                    let current_x = tile_x;

                    while (tile_x == current_x) && (tile_y <= bounds.max_y) {
                        entity_ids.nodes.extend(self.tile_local_ids(current_index, 0));
                        entity_ids.ways.extend(self.tile_local_ids(current_index, 1));
                        entity_ids.multipolygons.extend(self.tile_local_ids(current_index, 2));

                        current_index += 1;
                        if current_index >= tile_count {
                            break;
                        }
                        let (next_tile_x, next_tile_y) = self.tile_xy(current_index);
                        tile_x = next_tile_x;
                        tile_y = next_tile_y;
                    }

                    start_from_index = current_index;
                    bounds.min_x = current_x + 1;
                }
            }
        }
    }

    fn next_good_tile(&self, bounds: &mut tile::TileRange, start_index: usize) -> Option<usize> {
        let tile_count = self.tile_count();
        if start_index >= tile_count {
            return None;
        }

        let find_smallest_feasible_index = |from, min_x, min_y| {
            let large_enough = |idx| self.tile_xy(idx) >= (min_x, min_y);

            let mut lo = from;
            let mut hi = tile_count - 1;

            while lo < hi {
                let mid = (lo + hi) / 2;

                if large_enough(mid) {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }

            if large_enough(lo) {
                Some(lo)
            } else {
                None
            }
        };

        let mut idx = start_index;
        while let Some(next_idx) = find_smallest_feasible_index(idx, bounds.min_x, bounds.min_y) {
            let (tile_x, tile_y) = self.tile_xy(next_idx);
            if (tile_x, tile_y) > (bounds.max_x, bounds.max_y) {
                return None;
            }

            if tile_x == bounds.min_x {
                return Some(next_idx);
            }

            idx = next_idx;
            bounds.min_x = tile_x;
        }

        None
    }

    fn get_node(&'a self, idx: usize) -> Node<'a> {
        Node {
            entity: BaseOsmEntity {
                bytes: self.storages().node_storage.get_object(idx),
                reader: self,
            },
        }
    }

    fn get_way(&'a self, idx: usize) -> Way<'a> {
        let bytes = self.storages().way_storage.get_object(idx);
        let node_ids_start_pos = mem::size_of::<u64>();
        let node_ids = self.get_ints_by_ref(&bytes[node_ids_start_pos..]);
        Way {
            entity: BaseOsmEntity { bytes, reader: self },
            node_ids,
        }
    }

    fn get_polygon(&'a self, idx: usize) -> Polygon<'a> {
        let bytes = self.storages().polygon_storage.get_object(idx);
        let node_ids = self.get_ints_by_ref(bytes);
        Polygon { reader: self, node_ids }
    }

    fn get_multipolygon(&'a self, idx: usize) -> Multipolygon<'a> {
        let bytes = self.storages().multipolygon_storage.get_object(idx);
        let way_ids_start_pos = mem::size_of::<u64>();
        let way_ids = self.get_ints_by_ref(&bytes[way_ids_start_pos..]);
        Multipolygon {
            entity: BaseOsmEntity { bytes, reader: self },
            polygon_ids: way_ids,
        }
    }

    fn tile_xy(&self, idx: usize) -> (u32, u32) {
        let tile = self.storages().tile_storage.get_object(idx);
        let mut cursor = Cursor::new(tile);
        let x = cursor.read_u32::<LittleEndian>().unwrap();
        let y = cursor.read_u32::<LittleEndian>().unwrap();
        (x, y)
    }

    fn tile_local_ids(&self, idx: usize, local_ids_idx: usize) -> &'a [u32] {
        let tile = self.storages().tile_storage.get_object(idx);
        let offset = 2 * mem::size_of::<u32>() * (local_ids_idx + 1);
        self.get_ints_by_ref(&tile[offset..])
    }

    fn tile_count(&self) -> usize {
        self.storages().tile_storage.object_count
    }

    fn tags(&self, ref_bytes: &'a [u8]) -> Tags<'a> {
        Tags {
            kv_refs: self.get_ints_by_ref(ref_bytes),
            strings: self.storages().strings,
        }
    }

    fn get_ints_by_ref(&self, ref_bytes: &'a [u8]) -> &'a [u32] {
        let mut cursor = Cursor::new(ref_bytes);
        let offset = cursor.read_u32::<LittleEndian>().unwrap() as usize;
        let length = cursor.read_u32::<LittleEndian>().unwrap() as usize;
        &self.storages().ints[offset..offset + length]
    }

    fn storages(&self) -> &ObjectStorages<'a> {
        &self.storages
    }
}

fn filter_entities_by_ids<'a, E>(entities: impl Iterator<Item = E>, osm_ids: &Option<HashSet<u64>>) -> Vec<E>
where
    E: OsmEntity<'a> + Hash + Eq,
{
    match osm_ids {
        Some(ids) => entities.filter(|e| ids.contains(&e.global_id())).collect(),
        _ => entities.collect(),
    }
}

struct ObjectStorage<'a> {
    object_count: usize,
    object_size: usize,
    objects: &'a [u8],
}

impl<'a> ObjectStorage<'a> {
    fn from_bytes(bytes: &[u8], object_size: usize) -> (ObjectStorage<'_>, &[u8]) {
        let object_count = LittleEndian::read_u32(bytes) as usize;
        let object_start_pos = mem::size_of::<u32>();
        let object_end_pos = object_start_pos + object_size * object_count;
        let storage = ObjectStorage {
            object_count,
            object_size,
            objects: &bytes[object_start_pos..object_end_pos],
        };
        let rest = &bytes[object_end_pos..];
        (storage, rest)
    }

    fn get_object(&self, idx: usize) -> &'a [u8] {
        let start_pos = idx * self.object_size;
        let end_pos = start_pos + self.object_size;
        &self.objects[start_pos..end_pos]
    }
}

struct ObjectStorages<'a> {
    node_storage: ObjectStorage<'a>,
    way_storage: ObjectStorage<'a>,
    polygon_storage: ObjectStorage<'a>,
    multipolygon_storage: ObjectStorage<'a>,
    tile_storage: ObjectStorage<'a>,
    ints: &'a [u32],
    strings: &'a [u8],
}

const INT_REF_SIZE: usize = 2 * mem::size_of::<u32>();
const NODE_SIZE: usize = mem::size_of::<u64>() + 2 * mem::size_of::<f64>() + INT_REF_SIZE;
const POLYGON_SIZE: usize = INT_REF_SIZE;
const WAY_OR_MULTIPOLYGON_SIZE: usize = mem::size_of::<u64>() + 2 * INT_REF_SIZE;
const TILE_SIZE: usize = 2 * mem::size_of::<u32>() + 3 * INT_REF_SIZE;

impl<'a> ObjectStorages<'a> {
    // All geodata members have sizes divisible by 4, so the u8* -> u32* cast should be safe,
    // provided that `bytes` is aligned to 4 bytes (if it's not, we're in trouble anyway).
    #[cfg_attr(feature = "cargo-clippy", allow(clippy::cast_ptr_alignment))]
    fn from_bytes(bytes: &[u8]) -> ObjectStorages<'_> {
        let (node_storage, rest) = ObjectStorage::from_bytes(bytes, NODE_SIZE);
        let (way_storage, rest) = ObjectStorage::from_bytes(rest, WAY_OR_MULTIPOLYGON_SIZE);
        let (polygon_storage, rest) = ObjectStorage::from_bytes(rest, POLYGON_SIZE);
        let (multipolygon_storage, rest) = ObjectStorage::from_bytes(rest, WAY_OR_MULTIPOLYGON_SIZE);
        let (tile_storage, rest) = ObjectStorage::from_bytes(rest, TILE_SIZE);

        let int_count = LittleEndian::read_u32(rest) as usize;
        let start_pos = mem::size_of::<u32>();
        let end_pos = start_pos + mem::size_of::<u32>() * int_count;
        let byte_seq = &rest[start_pos..end_pos];
        let int_ptr = byte_seq.as_ptr() as *const u32;
        let ints = unsafe { slice::from_raw_parts(int_ptr, int_count) };
        let strings = &rest[end_pos..];

        ObjectStorages {
            node_storage,
            way_storage,
            polygon_storage,
            multipolygon_storage,
            tile_storage,
            ints,
            strings,
        }
    }
}

pub struct Tags<'a> {
    kv_refs: &'a [u32],
    strings: &'a [u8],
}

const KV_REF_SIZE: usize = 4;

pub struct StringWithOffset<'a> {
    pub str: &'a str,
    pub offset: usize,
}

impl<'a> Tags<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&'a str> {
        let kv_count = self.get_kv_count();
        if kv_count == 0 {
            return None;
        }
        let mut lo = 0;
        let mut hi = kv_count - 1;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (k, v) = self.get_kv(mid);
            match k.str.cmp(key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(v.str),
            }
        }
        let (k, v) = self.get_kv(lo);
        if k.str == key {
            Some(v.str)
        } else {
            None
        }
    }

    pub fn iter(&'a self) -> impl Iterator<Item = (StringWithOffset<'a>, StringWithOffset<'a>)> {
        (0..self.get_kv_count()).map(move |idx| self.get_kv(idx))
    }

    fn get_kv(&self, idx: usize) -> (StringWithOffset<'a>, StringWithOffset<'a>) {
        let start_idx = idx * KV_REF_SIZE;
        let get_str_with_offset = |offset| {
            let start_pos = self.kv_refs[start_idx + offset] as usize;
            let length = self.kv_refs[start_idx + offset + 1] as usize;
            StringWithOffset {
                str: self.get_str(start_pos, length),
                offset: start_pos,
            }
        };
        (get_str_with_offset(0), get_str_with_offset(2))
    }

    fn get_str(&self, start_pos: usize, length: usize) -> &'a str {
        unsafe { str::from_utf8_unchecked(&self.strings[start_pos..start_pos + length]) }
    }

    fn get_kv_count(&self) -> usize {
        self.kv_refs.len() / KV_REF_SIZE
    }
}

#[derive(Clone)]
struct BaseOsmEntity<'a> {
    bytes: &'a [u8],
    reader: &'a GeodataReader<'a>,
}

macro_rules! implement_osm_entity {
    ($type_name:ty) => {
        impl<'a> PartialEq for $type_name {
            fn eq(&self, other: &$type_name) -> bool {
                self.global_id() == other.global_id()
            }
        }

        impl<'a> Eq for $type_name {}

        impl<'a> Hash for $type_name {
            fn hash<H: Hasher>(&self, state: &mut H) {
                self.global_id().hash(state);
            }
        }

        impl<'a> OsmEntity<'a> for $type_name {
            fn global_id(&self) -> u64 {
                LittleEndian::read_u64(self.entity.bytes)
            }

            fn tags(&self) -> Tags<'a> {
                let entity = &self.entity;
                let start_pos = entity.bytes.len() - INT_REF_SIZE;
                entity.reader.tags(&entity.bytes[start_pos..])
            }
        }
    };
}

#[derive(Clone)]
pub struct Node<'a> {
    entity: BaseOsmEntity<'a>,
}

implement_osm_entity!(Node<'a>);

impl<'a> Coords for Node<'a> {
    fn lat(&self) -> f64 {
        let start_pos = mem::size_of::<u64>();
        LittleEndian::read_f64(&self.entity.bytes[start_pos..])
    }

    fn lon(&self) -> f64 {
        let start_pos = mem::size_of::<u64>() + mem::size_of::<f64>();
        LittleEndian::read_f64(&self.entity.bytes[start_pos..])
    }
}

pub struct Way<'a> {
    entity: BaseOsmEntity<'a>,
    node_ids: &'a [u32],
}

implement_osm_entity!(Way<'a>);

impl<'a> Way<'a> {
    pub fn node_count(&self) -> usize {
        self.node_ids.len()
    }

    pub fn get_node(&self, idx: usize) -> Node<'a> {
        let node_id = self.node_ids[idx];
        self.entity.reader.get_node(node_id as usize)
    }
}

impl<'a> OsmArea for Way<'a> {
    fn is_closed(&self) -> bool {
        if self.node_count() <= 2 {
            return false;
        }
        let first_node = self.get_node(0);
        let last_node = self.get_node(self.node_count() - 1);
        (first_node.lat(), first_node.lon()) == (last_node.lat(), last_node.lon())
    }
}

pub struct Polygon<'a> {
    reader: &'a GeodataReader<'a>,
    node_ids: &'a [u32],
}

impl<'a> Polygon<'a> {
    pub fn node_count(&self) -> usize {
        self.node_ids.len()
    }

    pub fn get_node(&self, idx: usize) -> Node<'a> {
        let node_id = self.node_ids[idx];
        self.reader.get_node(node_id as usize)
    }
}

pub struct Multipolygon<'a> {
    entity: BaseOsmEntity<'a>,
    polygon_ids: &'a [u32],
}

implement_osm_entity!(Multipolygon<'a>);

impl<'a> Multipolygon<'a> {
    pub fn polygon_count(&self) -> usize {
        self.polygon_ids.len()
    }

    pub fn get_polygon(&self, idx: usize) -> Polygon<'a> {
        let polygon_id = self.polygon_ids[idx];
        self.entity.reader.get_polygon(polygon_id as usize)
    }
}

impl<'a> OsmArea for Multipolygon<'a> {
    fn is_closed(&self) -> bool {
        true
    }
}
