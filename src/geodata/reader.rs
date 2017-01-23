use errors::*;

use std::cmp::Ordering;

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
    pub nodes: Vec<Node<'a>>,
}

pub struct GeodataReader<'a> {
    handle: GeodataHandle<'a>,
}

impl<'a> GeodataReader<'a> {
    pub fn new(file_name: &str) -> Result<GeodataReader> {
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

        while start_from_index < tiles.len() {
            let first_good_tile_index = next_good_tile(tiles, &bounds, start_from_index);

            if first_good_tile_index.is_none() {
                break;
            }

            let mut current_index = first_good_tile_index.unwrap();
            let mut current_tile = tiles.get(current_index);
            let current_x = current_tile.get_tile_x();

            while (current_tile.get_tile_x() == current_x) && (current_tile.get_tile_y() <= bounds.max_y) {
                for node_id in current_tile.get_local_node_ids().unwrap().iter() {
                    result.nodes.push(Node {
                        reader: self.get_reader().get_nodes().unwrap().get(node_id)
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

pub struct Node<'a> {
    reader: geodata_capnp::node::Reader<'a>
}

impl<'a> OsmEntity<'a> for Node<'a> {
    fn global_id(&self) -> u64 {
        self.reader.get_global_id()
    }

    fn tags(&self) -> Tags<'a> {
        Tags::new(self.reader.get_tags().unwrap())
    }
}

impl<'a> Coords for Node<'a> {
    fn lat(&self) -> f64 {
        self.reader.get_coords().unwrap().get_lat()
    }

    fn lon(&self) -> f64 {
        self.reader.get_coords().unwrap().get_lon()
    }
}

fn next_good_tile<'a>(tiles: struct_list::Reader<'a, geodata_capnp::tile::Owned>, bounds: &tile::TileRange, start_index: u32) -> Option<u32> {
    if start_index >= tiles.len() {
        return None;
    }

    let mut lo = start_index;
    let mut hi = tiles.len() - 1;

    let get_tile_xy = |idx| {
        let tile = tiles.get(idx);
        (tile.get_tile_x(), tile.get_tile_y())
    };

    let large_enough = |idx| get_tile_xy(idx) >= (bounds.min_x, bounds.min_y);
    let small_enough = |idx| get_tile_xy(idx) <= (bounds.max_x, bounds.max_y);

    while lo < hi {
        let mid = (lo + hi) / 2;

        if large_enough(mid) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }

    if large_enough(lo) && small_enough(lo) {
        Some(lo)
    } else {
        None
    }
}
