mod common;

use crate::common::get_test_path;
use renderer::mapcss::parser::parse_file;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

fn canonize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

#[test]
fn test_mapnik_parse() {
    let mapnik_base_path = get_test_path(&["mapcss"]);
    let mapnik_path = get_test_path(&["mapcss", "mapnik.mapcss"]);
    let rules = parse_file(Path::new(&mapnik_base_path), "mapnik.mapcss").unwrap();

    let rules_str = rules.iter().map(|x| format!("{}", x)).collect::<Vec<_>>().join("\n\n");
    let mapnik_path_parsed = PathBuf::from(&mapnik_path).with_extension("parsed");
    File::create(mapnik_path_parsed)
        .unwrap()
        .write_all(rules_str.as_bytes())
        .unwrap();

    let mut canonical_rules_str = String::new();
    let mapnik_path_canonical = PathBuf::from(mapnik_path).with_extension("parsed.canonical");
    File::open(mapnik_path_canonical)
        .unwrap()
        .read_to_string(&mut canonical_rules_str)
        .unwrap();
    assert_eq!(rules_str, canonize_newlines(&canonical_rules_str));
}

#[test]
fn test_parsing_is_idempotent() {
    let mapnik_base_path = get_test_path(&["mapcss"]);
    let mapnik_path = get_test_path(&["mapcss", "mapnik.parsed.canonical"]);

    let mut canonical = String::new();
    File::open(mapnik_path).unwrap().read_to_string(&mut canonical).unwrap();
    let rules = parse_file(Path::new(&mapnik_base_path), "mapnik.parsed.canonical").unwrap();

    let rules_str = rules.iter().map(|x| format!("{}", x)).collect::<Vec<_>>().join("\n\n");
    assert_eq!(rules_str, canonize_newlines(&canonical));
}
