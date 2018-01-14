extern crate renderer;
#[macro_use]
extern crate serde_derive;

mod common;

use std::fs::File;
use std::io::Read;
use renderer::draw::drawer::Drawer;
use renderer::mapcss::parser::Parser;
use renderer::mapcss::styler::Styler;
use renderer::mapcss::token::Tokenizer;

#[test]
fn test_rendering() {
    let nano_moscow = common::import_nano_moscow();
    let reader = renderer::geodata::reader::GeodataReader::new(&nano_moscow).unwrap();
    let tiles = common::read_tiles();

    let mut mapcss_content = String::new();
    File::open(common::get_test_path(&["mapcss", "mapnik.mapcss"]))
        .unwrap()
        .read_to_string(&mut mapcss_content)
        .unwrap();

    let styler = Styler::new(
        Parser::new(Tokenizer::new(&mapcss_content))
            .parse()
            .unwrap(),
    );
    let drawer = renderer::draw::pure_rust_drawer::PureRustDrawer::new();

    for tile in tiles {
        let tile_to_draw = renderer::tile::Tile {
            zoom: tile.zoom,
            x: tile.x,
            y: tile.y,
        };
        drawer
            .draw_tile(
                &reader.get_entities_in_tile(&tile_to_draw, &None),
                &tile_to_draw,
                &styler,
            )
            .unwrap();
    }
}
