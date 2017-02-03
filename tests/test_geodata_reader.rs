extern crate rustc_serialize;
use rustc_serialize::json;

extern crate capnp;
extern crate renderer;

use capnp::message::Builder;
use capnp::serialize;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufWriter, Read};
use renderer::geodata_capnp::geodata;
use renderer::geodata::reader::OsmEntity;

fn get_test_file(file_name: &str) -> String {
    let mut test_osm_path = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_osm_path.push("tests");
    test_osm_path.push(file_name);

    test_osm_path.to_str().unwrap().to_string()
}

#[derive(RustcDecodable)]
pub struct Tile {
    zoom: u8,
    x: u32,
    y: u32,
    nodes: Vec<u64>,
    ways: Vec<u64>,
    relations: Vec<u64>,
}

fn compare_ids(entity_type: &str, tile: &renderer::tile::Tile, actual: &BTreeSet<u64>, expected: &BTreeSet<u64>) {
    for e in expected.iter() {
        assert!(actual.contains(e), "{} {} is expected to be present in tile {:?}", entity_type, e, tile);
    }
    for a in actual.iter() {
        assert!(expected.contains(a), "Found an unexpected {} {} in tile {:?}", entity_type, a, tile);
    }
}

#[test]
fn test_synthetic_data() {
    let bin_file = get_test_file("synthetic.bin");

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
                // nothing is in the range fox x = 4
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
    let node_ids = reader.get_entities_in_tile(&tile).nodes.iter().map(|x| x.global_id()).collect::<BTreeSet<_>>();
    assert_eq!(good_node_ids, node_ids);
}

#[test]
fn test_nano_moscow_import() {
    let bin_file = get_test_file("nano_moscow.bin");
    renderer::geodata::importer::import(
        &get_test_file("nano_moscow.osm"),
        &bin_file
    ).unwrap();

    let mut test_data_file = File::open(&get_test_file("test_data.json")).unwrap();
    let mut test_data_content = String::new();
    test_data_file.read_to_string(&mut test_data_content).unwrap();
    let tiles: Vec<Tile> = json::decode(&test_data_content).unwrap();

    let reader = renderer::geodata::reader::GeodataReader::new(&bin_file).unwrap();

    for t in tiles {
        let tile = renderer::tile::Tile {
            zoom: t.zoom,
            x: t.x,
            y: t.y,
        };

        let tile_content = reader.get_entities_in_tile(&tile);

        let actual_nodes = tile_content.nodes.iter().map(|x| x.global_id()).collect::<BTreeSet<_>>();
        let expected_nodes = t.nodes.iter().map(|x| *x).collect::<BTreeSet<_>>();

        compare_ids("node", &tile, &actual_nodes, &expected_nodes);

        let actual_ways = tile_content.ways.iter().map(|x| x.global_id()).collect::<BTreeSet<_>>();
        let expected_ways = t.ways.iter().map(|x| *x).collect::<BTreeSet<_>>();

        compare_ids("way", &tile, &actual_ways, &expected_ways);

        let actual_relations = tile_content.relations.iter().map(|x| x.global_id()).collect::<BTreeSet<_>>();
        let expected_relations = t.relations.iter().map(|x| *x).collect::<BTreeSet<_>>();

        compare_ids("relation", &tile, &actual_relations, &expected_relations);
    }
}
