use errors::*;

use cs;
use geodata::reader::{OsmEntities, OsmEntity, Way};
use libc;
use mapcss::parser::*;
use mapcss::token::Color;
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
    if selector.layer_id.is_some() {
        return false;
    }

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
            let is_closed = way.get_node(0) == way.get_node(way.node_count() - 1);
            expected == is_closed
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

fn style_way<'a, 'b>(way: &Way<'a>, rules: &'b Vec<Rule>, zoom: u8) -> HashMap<String, &'b PropertyValue> {
    let mut result = HashMap::new();

    for rule in rules {
        if rule.selectors.iter().any(|x| way_matches(&way, x, zoom)) {
            for prop in rule.properties.iter() {
                result.insert(prop.name.clone(), &prop.value);
            }
        }
    }

    result
}

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &Tile, rules: &Vec<Rule>) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    unsafe {
        let s = cs::cairo_image_surface_create(cs::enums::Format::Rgb24, TILE_SIZE as i32, TILE_SIZE as i32);

        let cr = cs::cairo_create(s);

        let get_delta = |c| -((TILE_SIZE as f64) * (c as f64));
        cs::cairo_translate(cr, get_delta(tile.x), get_delta(tile.y));

        for w in entities.ways.iter() {
            if w.node_count() == 0 {
                continue;
            }

            let styles = style_way(w, rules, tile.zoom);

            let color = match styles.get("color") {
                Some(&&PropertyValue::Color(color)) => color,
                _ => continue,
            };

            let width = match styles.get("width") {
                Some(&&PropertyValue::Numbers(ref nums)) if nums.len() == 1 => {
                    nums[0]
                },
                _ => 1.0f64,
            };

            cs::cairo_set_source_rgb(cr, color.r as f64, color.g as f64, color.b as f64);
            cs::cairo_set_line_width(cr, width);

            let (x, y) = coords_to_float_xy(&w.get_node(0), tile.zoom);
            cs::cairo_move_to(cr, x, y);
            for i in 1..w.node_count() {
                let (x, y) = coords_to_float_xy(&w.get_node(i), tile.zoom);
                cs::cairo_line_to(cr, x, y);
            }
            cs::cairo_stroke(cr);
        }

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}
