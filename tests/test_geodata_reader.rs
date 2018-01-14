extern crate rustc_serialize;
use rustc_serialize::json;

extern crate capnp;
extern crate renderer;

mod common;

use capnp::message::Builder;
use capnp::serialize;
use common::{get_test_path, import_nano_moscow};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::cmp::Eq;
use std::fs::File;
use std::hash::Hash;
use std::io::{BufWriter, Read};
use renderer::geodata_capnp::geodata;
use renderer::geodata::reader::OsmEntity;

type Tags = HashMap<String, String>;
type IdsWithTags = HashMap<u64, Tags>;

#[derive(RustcDecodable)]
pub struct Tile {
    zoom: u8,
    x: u32,
    y: u32,
    nodes: IdsWithTags,
    ways: IdsWithTags,
    relations: IdsWithTags,
}

fn compare_ids<'a>(
    entity_type: &str,
    tile: &renderer::tile::Tile,
    actual: &BTreeSet<u64>,
    expected: &BTreeSet<u64>,
    actual_ids_with_tags: &HashMap<u64, renderer::geodata::reader::Tags<'a>>,
    expected_ids_with_tags: &IdsWithTags,
) {
    for e in expected.iter() {
        match actual.get(e) {
            Some(_) => {
                if let Some(expected_tags) = expected_ids_with_tags.get(e) {
                    let actual_tags = actual_ids_with_tags
                        .get(e)
                        .expect(&format!("Expected to have tags for {} {}", entity_type, e));
                    for (k, v) in expected_tags.iter() {
                        let actual_tag = actual_tags.get_by_key(k);
                        assert_eq!(
                            Some(v.as_ref()),
                            actual_tag,
                            "Expected {}={} for {} {}, found {:?}",
                            k,
                            v,
                            entity_type,
                            e,
                            actual_tag
                        );
                    }
                }
            }
            None => assert!(
                actual.contains(e),
                "{} {} is expected to be present in tile {:?}",
                entity_type,
                e,
                tile
            ),
        }
    }
    for a in actual.iter() {
        assert!(
            expected.contains(a),
            "Found an unexpected {} {} in tile {:?}",
            entity_type,
            a,
            tile
        );
    }
}

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

#[test]
fn test_nano_moscow_import() {
    let mut test_data_file = File::open(&get_test_path(&["osm", "test_data.json"])).unwrap();
    let mut test_data_content = String::new();
    test_data_file
        .read_to_string(&mut test_data_content)
        .unwrap();
    let tiles: Vec<Tile> = json::decode(&test_data_content).unwrap();

    let nano_moscow = import_nano_moscow();
    let reader = renderer::geodata::reader::GeodataReader::new(&nano_moscow).unwrap();

    for t in tiles {
        let tile = renderer::tile::Tile {
            zoom: t.zoom,
            x: t.x,
            y: t.y,
        };

        let tile_content = reader.get_entities_in_tile(&tile, &None);

        fn collect_ids_with_tags<'a, E>(
            entity: &HashSet<E>,
        ) -> HashMap<u64, renderer::geodata::reader::Tags<'a>>
        where
            E: Eq + Hash + OsmEntity<'a>,
        {
            entity
                .iter()
                .map(|x| (x.global_id(), x.tags()))
                .collect::<HashMap<_, _>>()
        }

        let actual_nodes = tile_content
            .nodes
            .iter()
            .map(|x| x.global_id())
            .collect::<BTreeSet<_>>();
        let actual_node_ids_with_tags = collect_ids_with_tags(&tile_content.nodes);
        let expected_nodes = t.nodes.iter().map(|x| *x.0).collect::<BTreeSet<_>>();

        compare_ids(
            "node",
            &tile,
            &actual_nodes,
            &expected_nodes,
            &actual_node_ids_with_tags,
            &t.nodes,
        );

        let actual_ways = tile_content
            .ways
            .iter()
            .map(|x| x.global_id())
            .collect::<BTreeSet<_>>();
        let actual_way_ids_with_tags = collect_ids_with_tags(&tile_content.ways);
        let expected_ways = t.ways.iter().map(|x| *x.0).collect::<BTreeSet<_>>();

        compare_ids(
            "way",
            &tile,
            &actual_ways,
            &expected_ways,
            &actual_way_ids_with_tags,
            &t.ways,
        );

        let actual_relations = tile_content
            .relations
            .iter()
            .map(|x| x.global_id())
            .collect::<BTreeSet<_>>();
        let actual_relation_ids_with_tags = collect_ids_with_tags(&tile_content.relations);
        let expected_relations = t.relations.iter().map(|x| *x.0).collect::<BTreeSet<_>>();

        compare_ids(
            "relation",
            &tile,
            &actual_relations,
            &expected_relations,
            &actual_relation_ids_with_tags,
            &t.relations,
        );
    }
}
