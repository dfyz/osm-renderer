use errors::*;

use std::cmp::Ordering;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

use capnp::Word;
use capnp::serialize::SliceSegments;
use capnp::{serialize, struct_list};
use capnp::message::{Reader, ReaderOptions};
use coords::Coords;
use geodata_capnp;
use memmap::{Mmap, Protection};
use owning_ref::OwningHandle;
use tile;

type GeodataHandle<'a> = OwningHandle<
    Box<Mmap>,
    OwningHandle<
        Box<Reader<SliceSegments<'a>>>,
        Box<geodata_capnp::geodata::Reader<'a>>
    >
>;

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

pub struct GeodataReader<'a> {
    handle: GeodataHandle<'a>,
}

unsafe impl<'a> Send for GeodataReader<'a> {}
unsafe impl<'a> Sync for GeodataReader<'a> {}

impl<'a> GeodataReader<'a> {
    pub fn new(file_name: &str) -> Result<GeodataReader<'a>> {
        let input_file = Mmap::open_path(file_name, Protection::Read)
            .chain_err(|| format!("Failed to map {} to memory", file_name))?;

        let handle = OwningHandle::try_new(
            Box::new(input_file),
            |x| {
                let message = serialize::read_message_from_words(
                    Word::bytes_to_words(unsafe{(&*x).as_slice()}),
                    ReaderOptions {
                        traversal_limit_in_words: u64::max_value(),
                        nesting_limit: i32::max_value(),
                    }
                )?;
                OwningHandle::try_new(
                    Box::new(message),
                    |y| unsafe{&*y}.get_root::<geodata_capnp::geodata::Reader>().map(Box::new)
                )
            }
        )
            .chain_err(|| format!("Failed to decode geodata from {}", file_name))?;

        Ok(GeodataReader {
            handle: handle,
        })
    }

    pub fn get_entities_in_tile(&'a self, t: &tile::Tile) -> OsmEntities<'a> {
        let tiles = self.get_reader().get_tiles().unwrap();
        let mut bounds = tile::tile_to_max_zoom_tile_range(&t);
        let mut start_from_index = 0;

        let mut result: OsmEntities<'a> = Default::default();

        let nodes = self.get_reader().get_nodes().unwrap();
        let ways = self.get_reader().get_ways().unwrap();
        let relations = self.get_reader().get_relations().unwrap();

        while start_from_index < tiles.len() {
            let first_good_tile_index = next_good_tile(tiles, &mut bounds, start_from_index);

            if first_good_tile_index.is_none() {
                break;
            }

            let mut current_index = first_good_tile_index.unwrap();
            let mut current_tile = tiles.get(current_index);
            let current_x = current_tile.get_tile_x();

            while (current_tile.get_tile_x() == current_x) && (current_tile.get_tile_y() <= bounds.max_y) {
                for node_id in current_tile.get_local_node_ids().unwrap().iter() {
                    result.nodes.insert(Node {
                        reader: nodes.get(node_id),
                    });
                }

                for way_id in current_tile.get_local_way_ids().unwrap().iter() {
                    result.ways.insert(Way {
                        geodata: self.get_reader(),
                        reader: ways.get(way_id),
                    });
                }

                for relation_id in current_tile.get_local_relation_ids().unwrap().iter() {
                    result.relations.insert(Relation {
                        geodata: self.get_reader(),
                        reader: relations.get(relation_id),
                    });
                }

                current_index += 1;
                if current_index >= tiles.len() {
                    break;
                }
                current_tile = tiles.get(current_index);
            }

            start_from_index = current_index;
            bounds.min_x = current_x + 1;
        }

        result
    }

    fn get_reader(&self) -> &geodata_capnp::geodata::Reader {
        &self.handle
    }
}

pub struct Tags<'a> {
    reader: struct_list::Reader<'a, geodata_capnp::tag::Owned>
}

impl<'a> Tags<'a> {
    pub fn get_by_key(&self, key: &str) -> Option<&'a str> {
        if self.reader.len() == 0 {
            return None;
        }
        let mut lo = 0;
        let mut hi = self.reader.len() - 1;
        while lo < hi {
            let mid = (lo + hi) / 2;
            let mid_value = self.reader.get(mid);
            match mid_value.get_key().unwrap().cmp(key) {
                Ordering::Less => lo = mid + 1,
                Ordering::Greater => hi = mid,
                Ordering::Equal => return Some(mid_value.get_value().unwrap()),
            }
        }
        None
    }

    pub fn get(&self, index: u32) -> geodata_capnp::tag::Reader<'a> {
        self.reader.get(index)
    }

    pub fn len(&self) -> u32 {
        self.reader.len()
    }

    pub fn new(reader: geodata_capnp::tag_list::Reader<'a>) -> Tags {
        Tags {
            reader: reader.get_tags().unwrap(),
        }
    }
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
                self.reader.get_global_id()
            }

            fn tags(&self) -> Tags<'a> {
                Tags::new(self.reader.get_tags().unwrap())
            }
        }
    }
}

pub struct Node<'a> {
    reader: geodata_capnp::node::Reader<'a>,
}

implement_osm_entity!(Node<'a>);

impl<'a> Coords for Node<'a> {
    fn lat(&self) -> f64 {
        self.reader.get_coords().unwrap().get_lat()
    }

    fn lon(&self) -> f64 {
        self.reader.get_coords().unwrap().get_lon()
    }
}

pub struct Way<'a> {
    geodata: &'a geodata_capnp::geodata::Reader<'a>,
    reader: geodata_capnp::way::Reader<'a>,
}

implement_osm_entity!(Way<'a>);

macro_rules! implement_node_methods {
    () => {
        pub fn node_count(&self) -> u32 {
            self.reader.get_local_node_ids().unwrap().len()
        }

        pub fn get_node(&self, index: u32) -> Node<'a> {
            let node_id = self.reader.get_local_node_ids().unwrap().get(index);
            Node {
                reader: self.geodata.get_nodes().unwrap().get(node_id),
            }
        }
    }
}

impl<'a> Way<'a> {
    implement_node_methods!();
}

pub struct Relation<'a> {
    geodata: &'a geodata_capnp::geodata::Reader<'a>,
    reader: geodata_capnp::relation::Reader<'a>,
}

implement_osm_entity!(Relation<'a>);

impl<'a> Relation<'a> {
    implement_node_methods!();

    pub fn way_count(&self) -> u32 {
        self.reader.get_local_way_ids().unwrap().len()
    }

    pub fn get_way(&self, index: u32) -> Way<'a> {
        let way_id = self.reader.get_local_way_ids().unwrap().get(index);
        Way {
            geodata: self.geodata,
            reader: self.geodata.get_ways().unwrap().get(way_id),
        }
    }
}

fn next_good_tile<'a>(tiles: struct_list::Reader<'a, geodata_capnp::tile::Owned>, bounds: &mut tile::TileRange, start_index: u32) -> Option<u32> {
    if start_index >= tiles.len() {
        return None;
    }

    let get_tile_xy = |idx| {
        let tile = tiles.get(idx);
        (tile.get_tile_x(), tile.get_tile_y())
    };

    let find_smallest_feasible_index = |from, min_x, min_y| {
        let large_enough = |idx| get_tile_xy(idx) >= (min_x, min_y);

        let mut lo = from;
        let mut hi = tiles.len() - 1;

        while lo < hi {
            let mid = (lo + hi) / 2;

            if large_enough(mid) {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }

        if large_enough(lo) { Some(lo) } else { None }
    };

    let mut current_index = start_index;
    while let Some(next_index) = find_smallest_feasible_index(current_index, bounds.min_x, bounds.min_y) {
        if get_tile_xy(next_index) > (bounds.max_x, bounds.max_y) {
            return None;
        }

        let found_x = tiles.get(next_index).get_tile_x();
        if found_x == bounds.min_x {
            return Some(next_index);
        }

        current_index = next_index;
        bounds.min_x = found_x;
    }

    None
}
