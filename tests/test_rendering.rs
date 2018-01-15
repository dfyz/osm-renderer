extern crate png;
extern crate renderer;
#[macro_use]
extern crate serde_derive;

mod common;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use renderer::draw::tile_pixels::{dimension, RgbTriples};
use renderer::draw::png_writer::rgb_triples_to_png;
use renderer::mapcss::parser::Parser;
use renderer::mapcss::styler::Styler;
use renderer::mapcss::token::Tokenizer;

const RED_PIXEL: (u8, u8, u8) = (255, 0, 0);

fn read_png(file_name: &str) -> (RgbTriples, png::OutputInfo) {
    let decoder = png::Decoder::new(File::open(file_name).unwrap());
    let (info, mut reader) = decoder.read_info().unwrap();
    let mut result = RgbTriples::new();
    while let Some(row) = reader.next_row().unwrap() {
        result.extend(row.chunks(3).map(|v| (v[0], v[1], v[2])))
    }
    (result, info)
}

fn compare_png_outputs(zoom: u8) {
    let (expected, expected_info) = read_png(&common::get_test_path(&[
        "rendered",
        &format!("{}_expected.png", zoom),
    ]));
    let (actual, actual_info) = read_png(&common::get_test_path(&[
        "rendered",
        &format!("{}.png", zoom),
    ]));

    assert_eq!(
        expected_info.width,
        actual_info.width,
        "different widths for zoom level {}",
        zoom
    );
    assert_eq!(
        expected_info.height,
        actual_info.height,
        "different heights for zoom level {}",
        zoom
    );

    let diff = expected
        .iter()
        .zip(actual)
        .map(|(e, a)| {
            if *e != a {
                RED_PIXEL
            } else {
                Default::default()
            }
        })
        .collect::<Vec<_>>();
    let has_diff = diff.contains(&RED_PIXEL);

    if has_diff {
        let diff_output_path = common::get_test_path(&["rendered", &format!("{}_diff.png", zoom)]);
        let diff_output = File::create(&diff_output_path);

        diff_output
            .unwrap()
            .write_all(&rgb_triples_to_png(&diff, actual_info.width as usize, actual_info.height as usize).unwrap())
            .unwrap();
        assert!(
            false,
            "the tiles for zoom level {} differ from the expected ones; see {} for more details",
            zoom,
            std::fs::canonicalize(diff_output_path).unwrap().to_str().unwrap()
        );
    }
}

#[ignore]
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

    let mut rendered_tiles: BTreeMap<u8, BTreeMap<u32, BTreeMap<u32, RgbTriples>>> =
        BTreeMap::new();
    for tile in tiles {
        let tile_to_draw = renderer::tile::Tile {
            zoom: tile.zoom,
            x: tile.x,
            y: tile.y,
        };
        let entities = reader.get_entities_in_tile(&tile_to_draw, &None);
        let rendered = drawer.draw_to_pixels(&entities, &tile_to_draw, &styler);
        rendered_tiles
            .entry(tile_to_draw.zoom)
            .or_insert_with(Default::default)
            .entry(tile_to_draw.y)
            .or_insert_with(Default::default)
            .insert(tile_to_draw.x, rendered);
    }

    for (zoom, y_x_rendered) in rendered_tiles {
        let mut rgb = RgbTriples::new();
        for x_rendered in y_x_rendered.values() {
            for sub_y in 0..dimension() {
                for rendered in x_rendered.values() {
                    if sub_y == 0 {
                        rgb.extend(std::iter::repeat(RED_PIXEL).take(dimension()));
                    } else {
                        rgb.extend(&rendered[sub_y * dimension()..(sub_y + 1) * dimension() - 1]);
                        rgb.push(RED_PIXEL);
                    }
                }
            }
        }

        let height = y_x_rendered.values().len() * dimension();
        let width = y_x_rendered.values().nth(0).unwrap().len() * dimension();
        let png_bytes = rgb_triples_to_png(&rgb, width, height);

        let png_output = File::create(common::get_test_path(&[
            "rendered",
            &format!("{}.png", zoom),
        ]));

        png_output.unwrap().write_all(&png_bytes.unwrap()).unwrap();

        compare_png_outputs(zoom);
    }
}
