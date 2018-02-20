use errors::*;

use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::fs::File;
use std::io::Cursor;
use std::mem;
use std::slice;
use std::str;

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use coords::Coords;
use memmap::{Mmap, MmapOptions};
use owning_ref::OwningHandle;
use tile;

pub trait OsmEntity<'a> {
    fn global_id(&self) -> u64;
    fn tags(&self) -> Tags<'a>;
}

#[derive(Default)]
pub struct OsmEntities<'a> {
    pub nodes: HashSet<Node<'a>>,
    pub ways: HashSet<Way<'a>>,
    pub relations: HashSet<Relation<'a>>,
}

pub trait OsmArea {
    fn is_closed(&self) -> bool;
}

pub struct GeodataReader<'a> {
    handle: GeodataHandle<'a>,
}

impl<'a> GeodataReader<'a> {
    pub fn new(file_name: &str) -> Result<GeodataReader<'a>> {
        let input_file = File::open(file_name)
            .chain_err(|| format!("Failed to open {} for memory mapping", file_name))?;
        let mmap = unsafe {
            MmapOptions::new()
                .map(&input_file)
                .chain_err(|| format!("Failed to map {} to memory", file_name))?
        };

        let handle = OwningHandle::try_new(Box::new(mmap), |mm| {
            ObjectStorages::from_bytes(unsafe { &*mm }).map(Box::new)
        })?;
        Ok(GeodataReader { handle })
    }

    pub fn get_entities_in_tile(
        &'a self,
        t: &tile::Tile,
        osm_ids: &Option<HashSet<u64>>,
    ) -> OsmEntities<'a> {
        let mut bounds = tile::tile_to_max_zoom_tile_range(t);
        let mut start_from_index = 0;

        let mut result: OsmEntities<'a> = Default::default();

        let tile_count = self.tile_count();
        while start_from_index < tile_count {
            match self.next_good_tile(&mut bounds, start_from_index) {
                None => break,
                Some(mut current_index) => {
                    let (mut tile_x, mut tile_y) = self.tile_xy(current_index);
                    let current_x = tile_x;

                    while (tile_x == current_x) && (tile_y <= bounds.max_y) {
                        for node_id in self.tile_local_ids(current_index, 0) {
                            let node = self.get_node(*node_id as usize);
                            insert_entity_if_needed(node, osm_ids, &mut result.nodes);
                        }

                        for way_id in self.tile_local_ids(current_index, 1) {
                            let way = self.get_way(*way_id as usize);
                            if way.node_count() > 0 {
                                insert_entity_if_needed(way, osm_ids, &mut result.ways);
                            }
                        }

                        for relation_id in self.tile_local_ids(current_index, 2) {
                            let relation = self.get_relation(*relation_id as usize);
                            if relation.way_count() > 0 {
                                insert_entity_if_needed(relation, osm_ids, &mut result.relations);
                            }
                        }

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

        result
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
            entity: BaseOsmEntity {
                bytes,
                reader: self,
            },
            node_ids,
        }
    }

    fn get_relation(&'a self, idx: usize) -> Relation<'a> {
        let bytes = self.storages().relation_storage.get_object(idx);
        let way_ids_start_pos = mem::size_of::<u64>();
        let way_ids = self.get_ints_by_ref(&bytes[way_ids_start_pos..]);
        Relation {
            entity: BaseOsmEntity {
                bytes,
                reader: self,
            },
            way_ids,
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
        &self.handle
    }
}

fn insert_entity_if_needed<'a, E>(
    entity: E,
    osm_ids: &Option<HashSet<u64>>,
    result: &mut HashSet<E>,
) where
    E: OsmEntity<'a> + Hash + Eq,
{
    let should_insert = match *osm_ids {
        Some(ref ids) => ids.contains(&entity.global_id()),
        None => true,
    };
    if should_insert {
        result.insert(entity);
    }
}

struct ObjectStorage<'a> {
    object_count: usize,
    object_size: usize,
    objects: &'a [u8],
}

impl<'a> ObjectStorage<'a> {
    fn from_bytes(bytes: &[u8], object_size: usize) -> Result<(ObjectStorage, &[u8])> {
        let object_count = LittleEndian::read_u32(bytes) as usize;
        let object_start_pos = mem::size_of::<u32>();
        let object_end_pos = object_start_pos + object_size * object_count;
        let storage = ObjectStorage {
            object_count,
            object_size,
            objects: &bytes[object_start_pos..object_end_pos],
        };
        let rest = &bytes[object_end_pos..];
        Ok((storage, rest))
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
    relation_storage: ObjectStorage<'a>,
    tile_storage: ObjectStorage<'a>,
    ints: &'a [u32],
    strings: &'a [u8],
}

const INT_REF_SIZE: usize = 2 * mem::size_of::<u32>();
const NODE_SIZE: usize = mem::size_of::<u64>() + 2 * mem::size_of::<f64>() + INT_REF_SIZE;
const WAY_OR_RELATION_SIZE: usize = mem::size_of::<u64>() + 2 * INT_REF_SIZE;
const TILE_SIZE: usize = 2 * mem::size_of::<u32>() + 3 * INT_REF_SIZE;

impl<'a> ObjectStorages<'a> {
    fn from_bytes(bytes: &[u8]) -> Result<ObjectStorages> {
        let (node_storage, rest) = ObjectStorage::from_bytes(bytes, NODE_SIZE)?;
        let (way_storage, rest) = ObjectStorage::from_bytes(rest, WAY_OR_RELATION_SIZE)?;
        let (relation_storage, rest) = ObjectStorage::from_bytes(rest, WAY_OR_RELATION_SIZE)?;
        let (tile_storage, rest) = ObjectStorage::from_bytes(rest, TILE_SIZE)?;

        let int_count = LittleEndian::read_u32(rest) as usize;
        let start_pos = mem::size_of::<u32>();
        let end_pos = start_pos + mem::size_of::<u32>() * int_count;
        let ints = unsafe {
            let byte_seq = &rest[start_pos..end_pos];
            let int_ptr = byte_seq.as_ptr() as *const u32;
            slice::from_raw_parts(int_ptr, int_count)
        };
        let strings = &rest[end_pos..];

        Ok(ObjectStorages {
            node_storage,
            way_storage,
            relation_storage,
            tile_storage,
            ints,
            strings,
        })
    }
}

type GeodataHandle<'a> = OwningHandle<Box<Mmap>, Box<ObjectStorages<'a>>>;

pub struct Tags<'a> {
    kv_refs: &'a [u32],
    strings: &'a [u8],
}

const KV_REF_SIZE: usize = 4;

impl<'a> Tags<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&'a str> {
        let kv_count = self.kv_refs.len() / KV_REF_SIZE;
        if kv_count == 0 {
            return None;
        }
        let mut lo = 0;
        let mut hi = kv_count - 1;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (k, v) = self.get_kv(mid);
            match k.cmp(key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(v),
            }
        }
        let (k, v) = self.get_kv(lo);
        if k == key {
            Some(v)
        } else {
            None
        }
    }

    fn get_kv(&self, idx: usize) -> (&'a str, &'a str) {
        let start_idx = idx * KV_REF_SIZE;
        let get_int = |offset| self.kv_refs[start_idx + offset] as usize;
        (
            self.get_str(get_int(0), get_int(1)),
            self.get_str(get_int(2), get_int(3)),
        )
    }

    fn get_str(&self, start_pos: usize, length: usize) -> &'a str {
        unsafe { str::from_utf8_unchecked(&self.strings[start_pos..start_pos + length]) }
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
                entity.reader.tags(&entity.bytes[start_pos ..])
            }
        }
    }
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
        self.node_count() > 2 && (self.get_node(0) == self.get_node(self.node_count() - 1))
    }
}

pub struct Relation<'a> {
    entity: BaseOsmEntity<'a>,
    way_ids: &'a [u32],
}

implement_osm_entity!(Relation<'a>);

impl<'a> Relation<'a> {
    pub fn way_count(&self) -> usize {
        self.way_ids.len()
    }

    pub fn get_way(&self, idx: usize) -> Way<'a> {
        let way_id = self.way_ids[idx];
        self.entity.reader.get_way(way_id as usize)
    }
}

impl<'a> OsmArea for Relation<'a> {
    fn is_closed(&self) -> bool {
        true
    }
}
