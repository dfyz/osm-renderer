use errors::*;

use cs;
use geodata::reader::{OsmEntities, OsmEntity, Way};
use libc;
use mapcss::color::{Color, from_color_name};
use mapcss::parser::*;
use std::collections::HashMap;
use std::slice;
use tile::{coords_to_float_xy, Tile, TILE_SIZE};

unsafe extern "C" fn write_func(closure: *mut libc::c_void, data: *mut u8, len: libc::c_uint) -> cs::enums::Status {
    let png_bytes: &mut Vec<u8> = &mut *(closure as *mut Vec<u8>);
    png_bytes.extend(slice::from_raw_parts(data, len as usize));
    cs::enums::Status::Success
}

fn way_matches_test<'a>(way: &Way<'a>, test: &Test) -> bool {
    let tags = way.tags();

    let is_true_value = |x| x == "yes" || x == "true" || x == "1";

    match test {
        &Test::Unary { ref tag_name, ref test_type } => {
            let tag_val = tags.get_by_key(tag_name);
            match test_type {
                &UnaryTestType::Exists => tag_val.is_some(),
                &UnaryTestType::NotExists => tag_val.is_none(),
                &UnaryTestType::True => match tag_val {
                    Some(x) if is_true_value(x) => true,
                    _ => false,
                },
                &UnaryTestType::False => match tag_val {
                    Some(x) if is_true_value(x) => false,
                    _ => true,
                },
            }
        },
        &Test::BinaryStringCompare { ref tag_name, ref value, ref test_type } => {
            let tag_val = tags.get_by_key(tag_name);
            match test_type {
                &BinaryStringTestType::Equal => tag_val == Some(value),
                &BinaryStringTestType::NotEqual => tag_val != Some(value),
            }
        },
        &Test::BinaryNumericCompare { ref tag_name, ref value, ref test_type } => {
            let tag_val = match tags.get_by_key(tag_name).map(|x| x.parse::<f64>()) {
                Some(Ok(x)) => x,
                _ => return false,
            };
            match test_type {
                &BinaryNumericTestType::Less => tag_val < *value,
                &BinaryNumericTestType::LessOrEqual => tag_val <= *value,
                &BinaryNumericTestType::Greater => tag_val > *value,
                &BinaryNumericTestType::GreaterOrEqual => tag_val >= *value,
            }
        },
    }
}

fn way_matches_single<'a>(way: &Way<'a>, selector: &SingleSelector, zoom: u8) -> bool {
    if let Some(min_zoom) = selector.min_zoom {
        if zoom < min_zoom {
            return false
        }
    }

    if let Some(max_zoom) = selector.max_zoom {
        if zoom > max_zoom {
            return false
        }
    }

    let good_object_type = match selector.object_type {
        ObjectType::Way { should_be_closed: None } => true,
        ObjectType::Way { should_be_closed: Some(expected) } => {
            expected == way.is_closed()
        },
        _ => return false,
    };

    good_object_type && selector.tests.iter().all(|x| way_matches_test(way, x))
}

fn way_matches<'a>(way: &Way<'a>, selector: &Selector, zoom: u8) -> bool {
    match selector {
        &Selector::Nested {..} => false,
        &Selector::Single(ref sel) => way_matches_single(way, &sel, zoom),
    }
}

type Style<'a> = HashMap<String, &'a PropertyValue>;
type Styles<'a> = Vec<Style<'a>>;

fn get_layer_id<'a>(selector: &'a Selector) -> &'a str {
    let single = match selector {
        &Selector::Single(ref single) => single,
        &Selector::Nested { ref child , .. } => child,
    };
    match single.layer_id {
        Some(ref id) => id,
        None => "default",
    }
}

fn style_way<'a, 'b>(way: &Way<'a>, rules: &'b Vec<Rule>, zoom: u8) -> Styles<'b> {
    let mut layer_to_style: HashMap<&str, Style<'b>> = HashMap::new();

    for rule in rules {
        for sel in rule.selectors.iter().filter(|x| way_matches(&way, x, zoom)) {
            let layer_id = get_layer_id(&sel);

            let update_layer = |layer: &mut Style<'b>| {
                for prop in rule.properties.iter() {
                    layer.insert(prop.name.clone(), &prop.value);
                }
            };

            {
                if !layer_to_style.contains_key(layer_id) {
                    let layer_pattern = match layer_to_style.get("*") {
                        Some(all_layers_style) => all_layers_style.clone(),
                        None => Default::default(),
                    };

                    layer_to_style.insert(layer_id, layer_pattern);
                }

                update_layer(layer_to_style.get_mut(layer_id).unwrap());
            }

            if layer_id == "*" {
                for (k, v) in layer_to_style.iter_mut() {
                    if k != &"*" {
                        update_layer(v);
                    }
                }
            }
        }
    }

    layer_to_style.into_iter().filter(|&(k, _)| k != "*").map(|(_, v)| v).collect::<Vec<_>>()
}

fn get_color<'a>(style: &Style<'a>, prop_name: &str) -> Option<Color> {
    match style.get(prop_name) {
        Some(&&PropertyValue::Color(color)) => Some(color),
        Some(&&PropertyValue::Identifier(ref id)) => from_color_name(id.as_str()),
        _ => None,
    }
}

fn get_opacity<'a>(style: &Style<'a>, prop_name: &str) -> f64 {
    match style.get(prop_name) {
        Some(&&PropertyValue::Numbers(ref nums)) if nums.len() == 1 => nums[0],
        _ => 1.0,
    }
}

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &Tile, rules: &Vec<Rule>) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    unsafe {
        let s = cs::cairo_image_surface_create(cs::enums::Format::Rgb24, TILE_SIZE as i32, TILE_SIZE as i32);

        let cr = cs::cairo_create(s);

        let get_delta = |c| -((TILE_SIZE as f64) * (c as f64));
        cs::cairo_translate(cr, get_delta(tile.x), get_delta(tile.y));

        let mut canvas_color = None;

        for r in rules.iter() {
            for selector in r.selectors.iter() {
                if let &Selector::Single(ref single) = selector {
                    if let ObjectType::Canvas = single.object_type {
                        for prop in r.properties.iter() {
                            if prop.name == "fill-color" {
                                if let PropertyValue::Color(color) = prop.value {
                                    canvas_color = Some(color);
                                }
                            }
                        }
                    }
                }
            }
        }

        let to_double_color = |u8_color| (u8_color as f64) / 255.0_f64;
        let set_color = |c: Color, a: f64| {
            cs::cairo_set_source_rgba(cr, to_double_color(c.r), to_double_color(c.g), to_double_color(c.b), a);
        };

        if let Some(color) = canvas_color {
            set_color(color, 1.0);
            cs::cairo_paint(cr);
        }

        let mut all_way_styles = Vec::new();

        for w in entities.ways.iter() {
            if w.node_count() == 0 {
                continue;
            }

            for style in style_way(w, rules, tile.zoom) {
                all_way_styles.push((w, style))
            }
        }

        fn get_z_index<'a, 'b>(way: &Way<'a>, style: &Style<'b>) -> f64 {
            match style.get("z-index") {
                Some(&&PropertyValue::Numbers(ref nums)) if nums.len() == 1 => {
                    nums[0]
                },
                _ => if way.is_closed() { 1.0 } else { 3.0 },
            }
        }

        all_way_styles.sort_by(|&(ref k1, ref v1), &(ref k2, ref v2)| get_z_index(k1, v1).partial_cmp(&get_z_index(k2, v2)).unwrap());

        for &(ref w, ref style) in all_way_styles.iter() {
            let color = get_color(style, "color");
            let fill_color = get_color(style, "fill-color");

            if color.is_none() && fill_color.is_none() {
                continue;
            }

            let width = match style.get("width") {
                Some(&&PropertyValue::Numbers(ref nums)) if nums.len() == 1 => {
                    nums[0]
                },
                _ => 1.0f64,
            };

            if let Some(&&PropertyValue::Numbers(ref nums)) = style.get("dashes") {
                cs::cairo_set_dash(cr, nums.as_ptr(), nums.len() as i32, 0.0);
            }

            match style.get("linejoin") {
                Some(&&PropertyValue::Identifier(ref s)) => match s.as_str() {
                    "round" => cs::cairo_set_line_join(cr, cs::enums::LineJoin::Round),
                    "miter" => cs::cairo_set_line_join(cr, cs::enums::LineJoin::Miter),
                    "bevel" => cs::cairo_set_line_join(cr, cs::enums::LineJoin::Bevel),
                    _ => {},
                },
                _ => {},
            }

            match style.get("linecap") {
                Some(&&PropertyValue::Identifier(ref s)) => match s.as_str() {
                    "none" => cs::cairo_set_line_cap(cr, cs::enums::LineCap::Butt),
                    "round" => cs::cairo_set_line_cap(cr, cs::enums::LineCap::Round),
                    "square" => cs::cairo_set_line_cap(cr, cs::enums::LineCap::Square),
                    _ => {},
                },
                _ => {},
            }

            let draw_path = || {
                cs::cairo_new_path(cr);

                cs::cairo_set_line_width(cr, width);

                let (x, y) = coords_to_float_xy(&w.get_node(0), tile.zoom);
                cs::cairo_move_to(cr, x, y);
                for i in 1..w.node_count() {
                    let (x, y) = coords_to_float_xy(&w.get_node(i), tile.zoom);
                    cs::cairo_line_to(cr, x, y);
                }
            };

            if let Some(c) = color {
                draw_path();
                set_color(c, get_opacity(style, "opacity"));
                cs::cairo_stroke(cr);
            }

            if w.is_closed() {
                if let Some(c) = fill_color {
                    draw_path();
                    set_color(c, get_opacity(style, "fill-opacity"));
                    cs::cairo_fill(cr);
                }
            }
        }

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}
