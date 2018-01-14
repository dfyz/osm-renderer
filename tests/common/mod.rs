#![allow(dead_code)]

extern crate renderer;

extern crate serde;
extern crate serde_json;

use std::fs::File;
use std::collections::HashMap;
use std::path::PathBuf;
use std::io::Read;

pub fn get_test_path(relative_path: &[&str]) -> String {
    let mut test_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    test_path.push("tests");
    for component in relative_path {
        test_path.push(component);
    }

    test_path.to_str().unwrap().to_string()
}

pub fn import_nano_moscow() -> String {
    let bin_file = get_test_path(&["osm", "nano_moscow.bin"]);
    renderer::geodata::importer::import(&get_test_path(&["osm", "nano_moscow.osm"]), &bin_file)
        .unwrap();

    bin_file
}

pub type Tags = HashMap<String, String>;
pub type IdsWithTags = HashMap<u64, Tags>;

#[derive(Deserialize)]
pub struct Tile {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
    pub nodes: IdsWithTags,
    pub ways: IdsWithTags,
    pub relations: IdsWithTags,
}

pub fn read_tiles() -> Vec<Tile> {
    let mut test_data_file = File::open(&get_test_path(&["osm", "test_data.json"])).unwrap();
    let mut test_data_content = String::new();
    test_data_file
        .read_to_string(&mut test_data_content)
        .unwrap();
    serde_json::from_str(&test_data_content).unwrap()
}
