extern crate rustc_serialize;
use rustc_serialize::json;

extern crate renderer;

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
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
fn test_geodata_reader() {
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
