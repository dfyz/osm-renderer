mod common;

use crate::common::get_test_path;
use renderer::geodata::reader::OsmEntity;
use renderer::mapcss::color::{from_color_name, Color};
use renderer::mapcss::parser::parse_file;
use renderer::mapcss::styler::{LineCap, Style, StyleType, Styler};
use renderer::tile::Tile;
use std::collections::HashMap;
use std::path::Path;

#[test]
fn test_styling() {
    let bin_file = get_test_path(&["osm", "nano_moscow.bin"]);
    renderer::geodata::importer::import(&get_test_path(&["osm", "nano_moscow.osm"]), &bin_file).unwrap();
    let reader = renderer::geodata::reader::GeodataReader::load(&bin_file).unwrap();
    let styler = Styler::new(
        parse_file(Path::new(&get_test_path(&["mapcss"])), "mapnik.mapcss").unwrap(),
        &StyleType::Josm,
        None,
    );

    let entities = reader.get_entities_in_tile_with_neighbors(
        &Tile {
            x: 158_458,
            y: 81_948,
            zoom: 18,
        },
        &None,
    );

    let named_ways = entities.ways.iter().filter(|x| x.tags().get_by_key("name").is_some());
    let styles = styler.style_entities(named_ways, 18, false);

    let get_styles = |id, name| {
        styles
            .iter()
            .filter(|&&(w, _)| w.global_id() == id && w.tags().get_by_key("name") == Some(name))
            .map(|&(_, ref s)| s)
            .collect::<Vec<_>>()
    };

    let s1 = get_styles(23_369_934, "Романов переулок");
    compare_with_josm_style(
        s1[0],
        false,
        "Cascade{ color:#bbbbbb; linecap:Keyword{round}; linejoin:Keyword{round}; width:16.0; z-index:-1.0; }",
    );
    compare_with_josm_style(
            s1[1],
            false,
            "Cascade{ color:Keyword{white}; dashes:[4.0, 2.0]; linecap:Keyword{round}; linejoin:Keyword{round}; width:13.0; }",
        );
    compare_with_josm_style(
        s1[2],
        false,
        "Cascade{ color:#6c70d5; dashes:[0.0, 12.0, 10.0, 152.0]; linejoin:Keyword{bevel}; width:1.0; z-index:15.0; }",
    );
    compare_with_josm_style(
        s1[3],
        false,
        "Cascade{ color:#6c70d5; dashes:[0.0, 12.0, 9.0, 153.0]; linejoin:Keyword{bevel}; width:2.0; z-index:15.1; }",
    );
    compare_with_josm_style(
        s1[4],
        false,
        "Cascade{ color:#6c70d5; dashes:[0.0, 18.0, 2.0, 154.0]; linejoin:Keyword{bevel}; width:3.0; z-index:15.2; }",
    );
    compare_with_josm_style(
        s1[5],
        false,
        "Cascade{ color:#6c70d5; dashes:[0.0, 18.0, 1.0, 155.0]; linejoin:Keyword{bevel}; width:4.0; z-index:15.3; }",
    );

    let s2 = get_styles(373_569_473, "Аллея Романов");
    compare_with_josm_style(
        s2[0],
        false,
        "Cascade{ color:Keyword{grey}; linecap:Keyword{round}; linejoin:Keyword{round}; width:9.0; z-index:-1.0; }",
    );
    compare_with_josm_style(
        s2[1],
        false,
        "Cascade{ color:#ededed; linecap:Keyword{round}; linejoin:Keyword{round}; width:8.0; }",
    );

    let building_josm_style =
            "Cascade{ color:#330066; fill-color:#bca9a9; fill-opacity:0.9; linejoin:Keyword{miter}; width:0.2; z-index:-900.0;";

    for &(id, name) in &[
        (31_497_212, "Бизнес-центр «Романов двор»"),
        (31_482_164, "Факультет искусств МГУ"),
        (44_642_919, "Факультет журналистики МГУ"),
    ] {
        compare_with_josm_style(get_styles(id, name)[0], true, building_josm_style);
    }
}

fn compare_with_josm_style(our_style: &Style, way_is_closed: bool, josm_style_str: &str) {
    let josm_style = from_josm_style(way_is_closed, josm_style_str);
    assert_styles_eq(our_style, &josm_style);
}

fn assert_styles_eq(our_style: &Style, josm_style: &Style) {
    assert_eq!(our_style.z_index, josm_style.z_index);
    assert_eq!(our_style.color, josm_style.color);
    assert_eq!(our_style.fill_color, josm_style.fill_color);
    assert_eq!(our_style.opacity, josm_style.opacity);
    assert_eq!(our_style.fill_opacity, josm_style.fill_opacity);
    assert_eq!(our_style.width, josm_style.width);
    assert_eq!(our_style.dashes, josm_style.dashes);
    assert_eq!(our_style.line_cap, josm_style.line_cap);
}

fn from_josm_style(way_is_closed: bool, style: &str) -> Style {
    let mut props = HashMap::new();
    for p in style
        .trim_start_matches("Cascade{ ")
        .trim_end_matches('}')
        .split(';')
        .map(|x| x.trim().splitn(2, ':').collect::<Vec<_>>())
    {
        if p.len() > 1 {
            props.insert(p[0], p[1]);
        }
    }

    let parse_color = |prop_name| {
        props.get(prop_name).map(|x| {
            if x.starts_with('#') {
                Color {
                    r: u8::from_str_radix(&x[1..3], 16).unwrap(),
                    g: u8::from_str_radix(&x[3..5], 16).unwrap(),
                    b: u8::from_str_radix(&x[5..7], 16).unwrap(),
                }
            } else {
                from_color_name(x.trim_start_matches("Keyword{").trim_end_matches('}')).unwrap()
            }
        })
    };

    let parse_num = |prop_name| props.get(prop_name).map(|x| x.parse().unwrap());

    Style {
        layer: None,
        z_index: parse_num("z-index").unwrap_or(if way_is_closed { 1.0 } else { 3.0 }),

        color: parse_color("color"),
        fill_color: parse_color("fill-color"),
        is_foreground_fill: false,
        background_color: None,
        opacity: parse_num("opacity"),
        fill_opacity: parse_num("fill-opacity"),

        width: parse_num("width"),
        dashes: props.get("dashes").map(|x| {
            x.trim_start_matches('[')
                .trim_end_matches(']')
                .split(", ")
                .map(|x| x.parse().unwrap())
                .collect::<Vec<_>>()
        }),
        line_cap: Some(
            props
                .get("linecap")
                .map(|x| match *x {
                    "Keyword{round}" => LineCap::Round,
                    _ => unreachable!(),
                })
                .unwrap_or(LineCap::Butt),
        ),

        casing_color: None,
        casing_width: None,
        casing_dashes: None,
        casing_line_cap: None,

        icon_image: None,
        fill_image: None,
        text_style: None,
    }
}
