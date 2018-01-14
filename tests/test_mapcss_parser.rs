#[macro_use]
extern crate serde_derive;
extern crate renderer;

mod common;

use common::get_test_path;
use renderer::mapcss::token::Tokenizer;
use renderer::mapcss::parser::Parser;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

#[test]
fn test_mapnik_parse() {
    let mapnik_path = get_test_path(&["mapcss", "mapnik.mapcss"]);

    let mut mapnik_content = String::new();
    File::open(&mapnik_path)
        .unwrap()
        .read_to_string(&mut mapnik_content)
        .unwrap();

    let tokenizer = Tokenizer::new(&mapnik_content);
    let mut parser = Parser::new(tokenizer);
    let rules = parser.parse().unwrap();

    let rules_str = rules
        .iter()
        .map(|x| format!("{}", x))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mapnik_path_parsed = PathBuf::from(&mapnik_path).with_extension("parsed");
    File::create(&mapnik_path_parsed)
        .unwrap()
        .write_all(rules_str.as_bytes())
        .unwrap();

    let mut canonical_rules_str = String::new();
    let mapnik_path_canonical = PathBuf::from(mapnik_path).with_extension("parsed.canonical");
    File::open(&mapnik_path_canonical)
        .unwrap()
        .read_to_string(&mut canonical_rules_str)
        .unwrap();
    assert_eq!(rules_str, canonical_rules_str);
}

#[test]
fn test_parsing_is_idempotent() {
    let mapnik_path = get_test_path(&["mapcss", "mapnik.parsed.canonical"]);

    let mut canonical = String::new();
    File::open(mapnik_path)
        .unwrap()
        .read_to_string(&mut canonical)
        .unwrap();
    let mut parser = Parser::new(Tokenizer::new(&canonical));

    let rules_str = parser
        .parse()
        .unwrap()
        .iter()
        .map(|x| format!("{}", x))
        .collect::<Vec<_>>()
        .join("\n\n");
    assert_eq!(rules_str, canonical);
}
