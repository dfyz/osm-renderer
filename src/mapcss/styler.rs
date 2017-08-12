use mapcss::color::{Color, from_color_name};
use mapcss::parser::*;

use geodata::reader::{OsmEntity, Way};
use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq)]
pub enum LineJoin {
    Round,
    Miter,
    Bevel,
}

#[derive(Debug, Eq, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug)]
pub struct Style {
    pub z_index: f64,

    pub color: Option<Color>,
    pub fill_color: Option<Color>,
    pub opacity: Option<f64>,
    pub fill_opacity: Option<f64>,

    pub width: Option<f64>,
    pub dashes: Option<Vec<f64>>,
    pub line_join: Option<LineJoin>,
    pub line_cap: Option<LineCap>,
}

pub struct Styler {
    pub canvas_fill_color: Option<Color>,

    rules: Vec<Rule>,
}

impl Styler {
    pub fn new(rules: Vec<Rule>) -> Styler {
        let canvas_fill_color = extract_canvas_fill_color(&rules);

        Styler {
            rules,
            canvas_fill_color,
        }
    }

    pub fn style_ways<'w, 'wp, I>(&self, ways: I, zoom: u8) -> Vec<(&'wp Way<'w>, Style)>
        where I: Iterator<Item=&'wp Way<'w>>
    {
        let mut styled_ways = ways.flat_map(|x| {
            let default_z_index = if x.is_closed() { 1.0 } else { 3.0 };
            self
                .style_way(x, zoom)
                .into_iter()
                .filter(|&(k, _)| k != "*")
                .map(move |(_, v)| (x, property_map_to_style(&v, default_z_index, x)))
        }).collect::<Vec<_>>();

        styled_ways.sort_by(|&(w1, ref s1), &(w2, ref s2)| {
            let cmp1 = (s1.z_index, w1.global_id());
            let cmp2 = (s2.z_index, w2.global_id());
            cmp1.partial_cmp(&cmp2).unwrap()
        });

        styled_ways
    }

    fn style_way<'r, 'w>(&'r self, way: &Way<'w>, zoom: u8) -> LayerToPropertyMap<'r> {
        let mut result: LayerToPropertyMap<'r> = HashMap::new();

        for rule in &self.rules {
            for sel in rule.selectors.iter().filter(|x| way_matches(way, x, zoom)) {
                let layer_id = get_layer_id(sel);

                let update_layer = |layer: &mut PropertyMap<'r>| {
                    for prop in &rule.properties {
                        layer.insert(prop.name.clone(), &prop.value);
                    }
                };

                {
                    // Can't use result.entry(...).or_insert_with(...) because we need to immutably borrow
                    // the result to compute the default value in or_insert_with(), and the map is already borrowed
                    // as mutable when we call entry().
                    if !result.contains_key(layer_id) {
                        let parent_layer = result.get("*").cloned().unwrap_or_default();
                        result.insert(layer_id, parent_layer);
                    }

                    update_layer(result.get_mut(layer_id).unwrap());
                }

                if layer_id == "*" {
                    for (_, v) in result.iter_mut().filter(|&(k, _)| k != &"*") {
                        update_layer(v);
                    }
                }
            }
        }

        result
    }
}

type LayerToPropertyMap<'r> = HashMap<&'r str, PropertyMap<'r>>;
type PropertyMap<'r> = HashMap<String, &'r PropertyValue>;

fn property_map_to_style<'r, 'w, E>(property_map: &PropertyMap<'r>, default_z_index: f64, osm_entity: &E) -> Style
    where E: OsmEntity<'w>
{
    let warn = |prop_name, msg| {
        if let Some(val) = property_map.get(prop_name) {
            warn!("Entity #{}, property \"{}\" (value {:?}): {}", osm_entity.global_id(), prop_name, val, msg);
        }
    };

    let get_color = |prop_name| match property_map.get(prop_name) {
        Some(&&PropertyValue::Color(ref color)) => Some(color.clone()),
        Some(&&PropertyValue::Identifier(ref id)) => {
            let color = from_color_name(id.as_str());
            if color.is_none() {
                warn(prop_name, "unknown color");
            }
            color
        },
        _ => {
            warn(prop_name, "expected a valid color");
            None
        },
    };

    let get_num = |prop_name| match property_map.get(prop_name) {
        Some(&&PropertyValue::Numbers(ref nums)) if nums.len() == 1 => Some(nums[0]),
        _ => {
            warn(prop_name, "expected a number");
            None
        },
    };

    let get_id = |prop_name| match property_map.get(prop_name) {
        Some(&&PropertyValue::Identifier(ref id)) => Some(id.as_str()),
        _ => {
            warn(prop_name, "expected an identifier");
            None
        },
    };

    let line_join = match get_id("linejoin") {
        Some("round") => Some(LineJoin::Round),
        Some("miter") => Some(LineJoin::Miter),
        Some("bevel") => Some(LineJoin::Bevel),
        _ => {
            warn("linejoin", "unknown line join value");
            None
        },
    };

    let line_cap = match get_id("linecap") {
        Some("none") => Some(LineCap::Butt),
        Some("round") => Some(LineCap::Round),
        Some("square") => Some(LineCap::Square),
        _ => {
            warn("linecap", "unknown line cap value");
            None
        },
    };

    let dashes = match property_map.get("dashes") {
        Some(&&PropertyValue::Numbers(ref nums)) => Some(nums.clone()),
        _ => {
            warn("dashes", "expected a sequence of numbers");
            None
        },
    };

    let z_index = get_num("z-index").unwrap_or(default_z_index);

    Style {
        z_index,

        color: get_color("color"),
        fill_color: get_color("fill-color"),
        opacity: get_num("opacity"),
        fill_opacity: get_num("fill-opacity"),

        width: get_num("width"),
        dashes,
        line_join,
        line_cap,
    }
}

fn extract_canvas_fill_color(rules: &[Rule]) -> Option<Color> {
    for r in rules {
        for selector in &r.selectors {
            if let Selector::Single(ref single) = *selector {
                if let ObjectType::Canvas = single.object_type {
                    for prop in r.properties.iter().filter(|x| x.name == "fill-color") {
                        if let PropertyValue::Color(ref color) = prop.value {
                            return Some(color.clone());
                        }
                    }
                }
            }
        }
    }
    None
}

fn way_matches_test<'w>(way: &Way<'w>, test: &Test) -> bool {
    let tags = way.tags();

    let is_true_value = |x| x == "yes" || x == "true" || x == "1";

    match *test {
        Test::Unary { ref tag_name, ref test_type } => {
            let tag_val = tags.get_by_key(tag_name);
            match *test_type {
                UnaryTestType::Exists => tag_val.is_some(),
                UnaryTestType::NotExists => tag_val.is_none(),
                UnaryTestType::True => match tag_val {
                    Some(x) if is_true_value(x) => true,
                    _ => false,
                },
                UnaryTestType::False => match tag_val {
                    Some(x) if is_true_value(x) => false,
                    _ => true,
                },
            }
        },
        Test::BinaryStringCompare { ref tag_name, ref value, ref test_type } => {
            let tag_val = tags.get_by_key(tag_name);
            match *test_type {
                BinaryStringTestType::Equal => tag_val == Some(value),
                BinaryStringTestType::NotEqual => tag_val != Some(value),
            }
        },
        Test::BinaryNumericCompare { ref tag_name, ref value, ref test_type } => {
            let tag_val = match tags.get_by_key(tag_name).map(|x| x.parse::<f64>()) {
                Some(Ok(x)) => x,
                _ => return false,
            };
            match *test_type {
                BinaryNumericTestType::Less => tag_val < *value,
                BinaryNumericTestType::LessOrEqual => tag_val <= *value,
                BinaryNumericTestType::Greater => tag_val > *value,
                BinaryNumericTestType::GreaterOrEqual => tag_val >= *value,
            }
        },
    }
}

fn way_matches_single<'w>(way: &Way<'w>, selector: &SingleSelector, zoom: u8) -> bool {
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

fn way_matches<'w>(way: &Way<'w>, selector: &Selector, zoom: u8) -> bool {
    match *selector {
        Selector::Nested {..} => false,
        Selector::Single(ref sel) => way_matches_single(way, sel, zoom),
    }
}

fn get_layer_id(selector: &Selector) -> &str {
    let single = match *selector {
        Selector::Single(ref single) => single,
        Selector::Nested { ref child , .. } => child,
    };
    match single.layer_id {
        Some(ref id) => id,
        None => "default",
    }
}