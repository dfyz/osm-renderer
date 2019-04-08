use png;
use renderer;

mod common;

use renderer::draw::png_writer::rgb_triples_to_png;
use renderer::draw::tile_pixels::RgbTriples;
use renderer::mapcss::parser::parse_file;
use renderer::mapcss::styler::{StyleType, Styler};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

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

fn compare_png_outputs(zoom: u8, suffix: &str) {
    let (expected, expected_info) = read_png(&common::get_test_path(&["rendered", &format!("{}{}_expected.png", zoom, suffix)]));
    let (actual, actual_info) = read_png(&common::get_test_path(&["rendered", &format!("{}{}.png", zoom, suffix)]));

    assert_eq!(
        expected_info.width, actual_info.width,
        "different widths for zoom level {}",
        zoom
    );
    assert_eq!(
        expected_info.height, actual_info.height,
        "different heights for zoom level {}",
        zoom
    );

    let diff = expected
        .iter()
        .zip(actual)
        .map(|(e, a)| if *e != a { RED_PIXEL } else { Default::default() })
        .collect::<Vec<_>>();
    let has_diff = diff.contains(&RED_PIXEL);

    if has_diff {
        let diff_output_path = common::get_test_path(&["rendered", &format!("{}{}_diff.png", zoom, suffix)]);
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

fn test_rendering_zoom(zoom: u8, min_x: u32, max_x: u32, min_y: u32, max_y: u32, scale: usize) {
    let bin_file = common::get_test_path(&["osm", &format!("nano_moscow_{}.bin", zoom)]);
    renderer::geodata::importer::import(&common::get_test_path(&["osm", "nano_moscow.osm"]), &bin_file).unwrap();
    let reader = renderer::geodata::reader::GeodataReader::load(&bin_file).unwrap();
    let base_path = common::get_test_path(&["mapcss"]);
    let styler = Styler::new(
        parse_file(Path::new(&base_path), "mapnik.mapcss").unwrap(),
        &StyleType::Josm,
        None,
    );
    let drawer = renderer::draw::drawer::Drawer::new(Path::new(&base_path));

    let mut rendered_tiles: BTreeMap<u8, BTreeMap<u32, BTreeMap<u32, RgbTriples>>> = BTreeMap::new();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let tile_to_draw = renderer::tile::Tile { zoom, x, y };
            let entities = reader.get_entities_in_tile_with_neighbors(&tile_to_draw, &None);
            let rendered = drawer.draw_to_pixels(&entities, &tile_to_draw, scale, &styler);
            rendered_tiles
                .entry(tile_to_draw.zoom)
                .or_insert_with(Default::default)
                .entry(tile_to_draw.y)
                .or_insert_with(Default::default)
                .insert(tile_to_draw.x, rendered.triples);
        }
    }

    let tile_dimension = 256 * scale;

    for (zoom, y_x_rendered) in rendered_tiles {
        let mut rgb = RgbTriples::new();
        for x_rendered in y_x_rendered.values() {
            for sub_y in 0..tile_dimension {
                for rendered in x_rendered.values() {
                    if sub_y == 0 {
                        rgb.extend(std::iter::repeat(RED_PIXEL).take(tile_dimension));
                    } else {
                        rgb.extend(&rendered[sub_y * tile_dimension..(sub_y + 1) * tile_dimension - 1]);
                        rgb.push(RED_PIXEL);
                    }
                }
            }
        }

        let height = y_x_rendered.values().len() * tile_dimension;
        let width = y_x_rendered.values().nth(0).unwrap().len() * tile_dimension;
        let png_bytes = rgb_triples_to_png(&rgb, width, height);

        let suffix = if scale > 1 {
            format!("_{}x", scale)
        } else {
            String::new()
        };

        let png_output = File::create(common::get_test_path(&["rendered", &format!("{}{}.png", zoom, suffix)]));

        png_output.unwrap().write_all(&png_bytes.unwrap()).unwrap();

        compare_png_outputs(zoom, &suffix);
    }
}

#[test]
fn test_zoom_14() {
    test_rendering_zoom(14, 9903, 9904, 5121, 5122, 1)
}

#[test]
fn test_zoom_15() {
    test_rendering_zoom(15, 19_807, 19_808, 10_243, 10_244, 1)
}

#[test]
fn test_zoom_16() {
    test_rendering_zoom(16, 39_614, 39_616, 20_486, 20_488, 1)
}

#[test]
fn test_zoom_17() {
    test_rendering_zoom(17, 79_228, 79_232, 40_973, 40_976, 1)
}

#[test]
fn test_zoom_18() {
    test_rendering_zoom(18, 158_457, 158_465, 81_946, 81_953, 1)
}

// Only test one zoom level for 2x scaling to save time.
#[test]
fn test_zoom_18_2x() {
    test_rendering_zoom(18, 158_457, 158_465, 81_946, 81_953, 2)
}

