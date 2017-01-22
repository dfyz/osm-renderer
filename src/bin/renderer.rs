extern crate capnp;
extern crate renderer;

use capnp::struct_list;
use renderer::geodata::reader::GeodataReader;
use renderer::tile;

fn next_good_tile<'a>(tiles: struct_list::Reader<'a, renderer::geodata_capnp::tile::Owned>, bounds: &tile::TileRange, start_index: u32) -> Option<u32> {
    let mut lo = start_index;
    let mut hi = tiles.len();

    let get_tile_xy = |idx| {
        let tile = tiles.get(idx);
        (tile.get_tile_x(), tile.get_tile_y())
    };

    let large_enough = |idx| get_tile_xy(idx) >= (bounds.min_x, bounds.min_y);
    let small_enough = |idx| get_tile_xy(idx) <= (bounds.max_x, bounds.max_y);

    while lo + 1 < hi {
        let mid = (lo + hi - 1) / 2;

        if large_enough(mid) {
            hi = mid + 1;
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

fn main() {
    let reader = GeodataReader::new("mow.bin").unwrap();

    let rdr = reader.get_reader();

    let t = tile::Tile {
        zoom: 0,
        x: 0,
        y: 0,
    };

    let mut bounds = tile::tile_to_max_zoom_tile_range(&t);

    let g_tiles = rdr.get_tiles().unwrap();
    let mut start = 0;

    let mut tile_count = 0;
    let mut node_count = 0;
    let mut way_count = 0;

    while start < g_tiles.len() {
        let first_good = next_good_tile(g_tiles, &bounds, start);

        if first_good.is_none() {
            break;
        }

        let mut current_index = first_good.unwrap();
        let mut current_tile = g_tiles.get(current_index);
        let current_x = current_tile.get_tile_x();
        while current_x == current_tile.get_tile_x() && bounds.max_y >= current_tile.get_tile_y() {
            node_count += current_tile.get_local_node_ids().unwrap().len();
            way_count += current_tile.get_local_way_ids().unwrap().len();
            tile_count += 1;

            current_index += 1;
            if current_index >= g_tiles.len() {
                break;
            }
            current_tile = g_tiles.get(current_index);
        }

        start = current_index;
        bounds.min_x = current_x + 1;
    }

    println!("{} tiles, {} named nodes, {} named ways", tile_count, node_count, way_count);
}
