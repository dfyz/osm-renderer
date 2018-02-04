extern crate capnp;
extern crate renderer;

mod common;

use capnp::message::Builder;
use capnp::serialize;
use common::get_test_path;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::BufWriter;
use renderer::geodata_capnp::geodata;
use renderer::geodata::reader::OsmEntity;

#[test]
fn test_synthetic_data() {
    let bin_file = get_test_path(&["osm", "synthetic.bin"]);

    let mut good_node_ids = BTreeSet::new();

    {
        let output_file = File::create(&bin_file).unwrap();
        let mut writer = BufWriter::new(output_file);

        let mut message = Builder::new_default();

        {
            let mut geodata = message.init_root::<geodata::Builder>();

            let mut tile_ids = Vec::new();

            {
                let mut add_tile = |x, y, good| {
                    let node_idx = tile_ids.len();
                    tile_ids.push((x, y));
                    if good {
                        good_node_ids.insert(node_idx as u64);
                    }
                };

                // y = {8, 9, 13} are in the range for x = 1
                add_tile(1, 7, false);
                add_tile(1, 8, true);
                add_tile(1, 9, true);
                add_tile(1, 13, true);
                // y = {10, 11, 15} is in the range for x = 2
                add_tile(2, 10, true);
                add_tile(2, 11, true);
                add_tile(2, 15, true);
                add_tile(2, 16, false);
                add_tile(2, 17, false);
                // nothing is in the range for x = 4
                add_tile(4, 1, false);
                add_tile(4, 4, false);
                // nothing is in the range for x = 5
                add_tile(5, 20, false);
                add_tile(5, 23, false);
                add_tile(5, 200, false);
                // y = {11, 12, 14} are in the range for x = 7
                add_tile(7, 6, false);
                add_tile(7, 11, true);
                add_tile(7, 12, true);
                add_tile(7, 14, true);
                add_tile(7, 16, false);
                add_tile(7, 17, false);
            }

            {
                let mut nodes = geodata.borrow().init_nodes(tile_ids.len() as u32);
                for idx in 0..tile_ids.len() {
                    let i = idx as u32;
                    let mut nd = nodes.borrow().get(i);
                    nd.set_global_id(idx as u64);

                    let mut c = nd.init_coords();
                    c.set_lat(1.0);
                    c.set_lon(1.0);
                }
            }

            {
                let mut tiles = geodata.borrow().init_tiles(tile_ids.len() as u32);
                for (idx, &(x, y)) in tile_ids.iter().enumerate() {
                    let i = idx as u32;
                    let mut tile = tiles.borrow().get(i);
                    tile.set_tile_x(x);
                    tile.set_tile_y(y);

                    let mut local_nodes = tile.init_local_node_ids(1);
                    local_nodes.set(0, i);
                }
            }
        }

        serialize::write_message(&mut writer, &message).unwrap();
    }

    let reader = renderer::geodata::reader::GeodataReader::new(&bin_file).unwrap();
    let tile = renderer::tile::Tile {
        zoom: 15,
        x: 0,
        y: 1,
    };
    let node_ids = reader
        .get_entities_in_tile(&tile, &None)
        .nodes
        .iter()
        .map(|x| x.global_id())
        .collect::<BTreeSet<_>>();
    assert_eq!(good_node_ids, node_ids);
}
