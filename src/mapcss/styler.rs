pub use mapcss::style::Style;

use mapcss::color::{from_color_name, Color};
use mapcss::parser::*;

use geodata::reader::{OsmArea, OsmEntity};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

pub fn is_non_trivial_cap(line_cap: &Option<LineCap>) -> bool {
    match *line_cap {
        Some(LineCap::Square) | Some(LineCap::Round) => true,
        _ => false,
    }
}

pub enum StyleType {
    Josm,
    MapsMe,
}

pub struct Styler {
    pub canvas_fill_color: Option<Color>,
    pub use_caps_for_dashes: bool,

    rules: Vec<Rule>,
}

impl Styler {
    pub fn new(rules: Vec<Rule>, style_type: &StyleType) -> Styler {
        let use_caps_for_dashes = match *style_type {
            StyleType::Josm => true,
            _ => false,
        };
        let canvas_fill_color = extract_canvas_fill_color(&rules, style_type);

        Styler {
            use_caps_for_dashes,
            canvas_fill_color,
            rules,
        }
    }

    pub fn style_areas<'e, 'wp, I, A>(&self, areas: I, zoom: u8) -> Vec<(&'wp A, Style)>
    where
        A: OsmArea + OsmEntity<'e>,
        I: Iterator<Item = &'wp A>,
    {
        let mut styled_areas = areas
            .flat_map(|x| {
                let default_z_index = if x.is_closed() { 1.0 } else { 3.0 };
                self.style_area(x, zoom)
                    .into_iter()
                    .filter(|&(k, _)| k != "*")
                    .map(move |(_, v)| (x, property_map_to_style(&v, default_z_index, x)))
            })
            .collect::<Vec<_>>();

        styled_areas.sort_by(|&(w1, ref s1), &(w2, ref s2)| {
            let cmp1 = (s1.is_foreground_fill, s1.z_index, w1.global_id());
            let cmp2 = (s2.is_foreground_fill, s2.z_index, w2.global_id());
            cmp1.partial_cmp(&cmp2).unwrap()
        });

        styled_areas
    }

    fn style_area<'r, 'e, A>(&'r self, area: &A, zoom: u8) -> LayerToPropertyMap<'r>
    where
        A: OsmArea + OsmEntity<'e>,
    {
        let mut result: LayerToPropertyMap<'r> = HashMap::new();

        for rule in &self.rules {
            for sel in rule.selectors
                .iter()
                .filter(|x| area_matches(area, x, zoom))
            {
                let layer_id = get_layer_id(sel);

                let update_layer = |layer: &mut PropertyMap<'r>| {
                    for prop in &rule.properties {
                        layer.insert(prop.name.clone(), &prop.value);
                    }
                };

                {
                    // Can't use result.entry(...).or_insert_with(...) because we need to immutably
                    // borrow the result to compute the default value in or_insert_with(), and the
                    // map is already borrowed as mutable when we call entry().
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

fn property_map_to_style<'r, 'e, E>(
    property_map: &PropertyMap<'r>,
    default_z_index: f64,
    osm_entity: &E,
) -> Style
where
    E: OsmEntity<'e>,
{
    let warn = |prop_name, msg| {
        if let Some(val) = property_map.get(prop_name) {
            eprintln!(
                "Entity #{}, property \"{}\" (value {:?}): {}",
                osm_entity.global_id(),
                prop_name,
                val,
                msg
            );
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
        }
        _ => {
            warn(prop_name, "expected a valid color");
            None
        }
    };

    let get_num = |prop_name| match property_map.get(prop_name) {
        Some(&&PropertyValue::Numbers(ref nums)) if nums.len() == 1 => Some(nums[0]),
        _ => {
            warn(prop_name, "expected a number");
            None
        }
    };

    let get_id = |prop_name| match property_map.get(prop_name) {
        Some(&&PropertyValue::Identifier(ref id)) => Some(id.as_str()),
        _ => {
            warn(prop_name, "expected an identifier");
            None
        }
    };

    let line_cap = match get_id("linecap") {
        Some("none") | Some("butt") => Some(LineCap::Butt),
        Some("round") => Some(LineCap::Round),
        Some("square") => Some(LineCap::Square),
        _ => {
            warn("linecap", "unknown line cap value");
            None
        }
    };

    let dashes = match property_map.get("dashes") {
        Some(&&PropertyValue::Numbers(ref nums)) => Some(nums.clone()),
        _ => {
            warn("dashes", "expected a sequence of numbers");
            None
        }
    };

    let z_index = get_num("z-index").unwrap_or(default_z_index);

    let is_foreground_fill = match property_map.get("fill-position") {
        Some(&&PropertyValue::Identifier(ref id)) if *id == "background" => false,
        _ => true,
    };

    Style {
        z_index,

        color: get_color("color"),
        fill_color: get_color("fill-color"),
        is_foreground_fill,
        background_color: get_color("background-color"),
        opacity: get_num("opacity"),
        fill_opacity: get_num("fill-opacity"),

        width: get_num("width"),
        dashes,
        line_cap,
    }
}

fn extract_canvas_fill_color(rules: &[Rule], style_type: &StyleType) -> Option<Color> {
    let color_prop = match *style_type {
        StyleType::Josm => "fill-color",
        StyleType::MapsMe => "background-color",
    };
    for r in rules {
        for selector in &r.selectors {
            if let ObjectType::Canvas = selector.object_type {
                for prop in r.properties.iter().filter(|x| x.name == *color_prop) {
                    if let PropertyValue::Color(ref color) = prop.value {
                        return Some(color.clone());
                    }
                }
            }
        }
    }
    None
}

fn matches_by_tags<'e, E>(entity: &E, test: &Test) -> bool
where
    E: OsmEntity<'e>,
{
    let tags = entity.tags();

    let is_true_value = |x| x == "yes" || x == "true" || x == "1";

    match *test {
        Test::Unary {
            ref tag_name,
            ref test_type,
        } => {
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
        }
        Test::BinaryStringCompare {
            ref tag_name,
            ref value,
            ref test_type,
        } => {
            let tag_val = tags.get_by_key(tag_name);
            match *test_type {
                BinaryStringTestType::Equal => tag_val == Some(value),
                BinaryStringTestType::NotEqual => tag_val != Some(value),
            }
        }
        Test::BinaryNumericCompare {
            ref tag_name,
            ref value,
            ref test_type,
        } => {
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
        }
    }
}

fn area_matches<'e, A>(area: &A, selector: &Selector, zoom: u8) -> bool
where
    A: OsmArea + OsmEntity<'e>,
{
    if let Some(min_zoom) = selector.min_zoom {
        if zoom < min_zoom {
            return false;
        }
    }

    if let Some(max_zoom) = selector.max_zoom {
        if zoom > max_zoom {
            return false;
        }
    }

    let good_object_type = match selector.object_type {
        ObjectType::Way => true,
        ObjectType::Area => area.is_closed(),
        _ => false,
    };

    good_object_type && selector.tests.iter().all(|x| matches_by_tags(area, x))
}

fn get_layer_id(selector: &Selector) -> &str {
    match selector.layer_id {
        Some(ref id) => id,
        None => "default",
    }
}
