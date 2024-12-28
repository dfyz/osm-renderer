use crate::mapcss::color::{from_color_name, Color};
use crate::mapcss::parser::*;
use crate::mapcss::style_cache::StyleCache;

use crate::geodata::reader::{Multipolygon, Node, OsmArea, OsmEntity, Way};
use indexmap::IndexMap;
use std::cmp::Ordering;
use std::sync::Arc;
use std::sync::RwLock;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TextPosition {
    Center,
    Line,
}

pub fn is_non_trivial_cap(line_cap: &Option<LineCap>) -> bool {
    matches!(*line_cap, Some(LineCap::Square) | Some(LineCap::Round))
}

pub enum StyleType {
    Josm,
    MapsMe,
}

pub trait StyleableEntity {
    fn default_z_index(&self) -> f64;
    fn matches_object_type(&self, object_type: &ObjectType) -> bool;
}

pub trait CacheableEntity {
    fn cache_slot(&self) -> usize;
}

pub struct TextStyle {
    pub text: String,
    pub text_color: Option<Color>,
    pub text_position: Option<TextPosition>,
    pub font_size: Option<f64>,
}

pub struct Style {
    pub layer: Option<i64>,
    pub z_index: f64,

    pub color: Option<Color>,
    pub fill_color: Option<Color>,
    pub is_foreground_fill: bool,
    pub background_color: Option<Color>,
    pub opacity: Option<f64>,
    pub fill_opacity: Option<f64>,

    pub width: Option<f64>,
    pub dashes: Option<Vec<f64>>,
    pub line_cap: Option<LineCap>,

    pub casing_color: Option<Color>,
    pub casing_width: Option<f64>,
    pub casing_dashes: Option<Vec<f64>>,
    pub casing_line_cap: Option<LineCap>,

    pub icon_image: Option<String>,
    pub fill_image: Option<String>,
    pub text_style: Option<TextStyle>,
}

pub struct Styler {
    pub canvas_fill_color: Option<Color>,
    pub use_caps_for_dashes: bool,

    casing_width_multiplier: f64,
    font_size_multiplier: Option<f64>,
    rules: Vec<Rule>,

    style_cache: RwLock<StyleCache>,
}

pub enum StyledArea<'a, 'wr>
where
    'a: 'wr,
{
    Way(&'wr Way<'a>),
    Multipolygon(&'wr Multipolygon<'a>),
}

impl Styler {
    pub fn new(rules: Vec<Rule>, style_type: &StyleType, font_size_multiplier: Option<f64>) -> Styler {
        let use_caps_for_dashes = matches!(*style_type, StyleType::Josm);
        let canvas_fill_color = extract_canvas_fill_color(&rules, style_type);

        let casing_width_multiplier = match *style_type {
            StyleType::MapsMe => 1.0,
            _ => 2.0,
        };

        let style_cache = StyleCache::new(&rules);

        Styler {
            use_caps_for_dashes,
            canvas_fill_color,
            casing_width_multiplier,
            font_size_multiplier,
            rules,
            style_cache: RwLock::new(style_cache),
        }
    }

    pub fn style_entities<'e, 'wp, I, A>(&self, areas: I, zoom: u8, for_labels: bool) -> Vec<(&'wp A, Arc<Style>)>
    where
        A: CacheableEntity + StyleableEntity + OsmEntity<'e>,
        I: Iterator<Item = &'wp A>,
    {
        let mut styled_areas = Vec::new();
        for area in areas {
            let mut add_styles = |styles: &Vec<Arc<Style>>| {
                for s in styles.iter() {
                    styled_areas.push((area, Arc::clone(s)));
                }
            };

            {
                let read_cache = self.style_cache.read().unwrap();
                if let Some(styles) = read_cache.get(area, zoom) {
                    add_styles(&styles);
                    continue;
                }
            }

            let default_z_index = area.default_z_index();

            let all_property_maps = self.style_area(area, zoom);

            let base_layer = all_property_maps
                .iter()
                .find(|kvp| *kvp.0 == BASE_LAYER_NAME)
                .map(|kvp| kvp.1);

            let mut styles = Vec::new();
            for (layer, prop_map) in &all_property_maps {
                if *layer != "*" {
                    styles.push(Arc::new(property_map_to_style(
                        prop_map,
                        base_layer,
                        default_z_index,
                        self.casing_width_multiplier,
                        &self.font_size_multiplier,
                        area,
                    )))
                }
            }

            add_styles(&styles);
            self.style_cache.write().unwrap().insert(area, zoom, styles)
        }

        styled_areas.sort_by(|a, b| compare_styled_entities(a, b, for_labels));

        styled_areas
    }

    pub fn style_areas<'a, 'wr>(
        &self,
        ways: impl Iterator<Item = &'wr Way<'a>>,
        multipolygons: impl Iterator<Item = &'wr Multipolygon<'a>>,
        zoom: u8,
        for_labels: bool,
    ) -> Vec<(StyledArea<'a, 'wr>, Arc<Style>)> {
        let styled_ways = self.style_entities(ways, zoom, for_labels);
        let styled_multipolygons = self.style_entities(multipolygons, zoom, for_labels);

        let mut mp_iter = styled_multipolygons.into_iter();
        let mut way_iter = styled_ways.into_iter();
        let mut poly = mp_iter.next();
        let mut way = way_iter.next();
        let mut result = Vec::new();
        loop {
            let is_rel_better = {
                match (&poly, &way) {
                    (None, None) => break,
                    (Some(_), None) => true,
                    (None, Some(_)) => false,
                    (Some(mp), Some(way)) => compare_styled_entities(mp, way, for_labels) != Ordering::Greater,
                }
            };
            if is_rel_better {
                let (mp, style) = poly.unwrap();
                result.push((StyledArea::Multipolygon(mp), style));
                poly = mp_iter.next();
            } else {
                let (w, style) = way.unwrap();
                result.push((StyledArea::Way(w), style));
                way = way_iter.next();
            }
        }
        result
    }

    fn style_area<'r, 'e, A>(&'r self, area: &A, zoom: u8) -> LayerToPropertyMap<'r>
    where
        A: StyleableEntity + OsmEntity<'e>,
    {
        let mut result: LayerToPropertyMap<'r> = IndexMap::new();

        for rule in &self.rules {
            for sel in rule.selectors.iter().filter(|x| area_matches(area, x, zoom)) {
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

#[expect(clippy::float_cmp)]
fn compare_styled_entities<'a, E1, E2>(
    (a, a_style): &(&E1, Arc<Style>),
    (b, b_style): &(&E2, Arc<Style>),
    for_labels: bool,
) -> Ordering
where
    E1: OsmEntity<'a>,
    E2: OsmEntity<'a>,
{
    let get_layer = |s: &Style| s.layer.unwrap_or(0);

    let (a_layer, b_layer) = (get_layer(a_style), get_layer(b_style));

    if a_layer != b_layer {
        return a_layer.cmp(&b_layer);
    }

    if !for_labels && a_style.is_foreground_fill != b_style.is_foreground_fill {
        return a_style.is_foreground_fill.cmp(&b_style.is_foreground_fill);
    }

    if a_style.z_index != b_style.z_index {
        return a_style.z_index.partial_cmp(&b_style.z_index).unwrap();
    }

    a.global_id().cmp(&b.global_id())
}

type LayerToPropertyMap<'r> = IndexMap<&'r str, PropertyMap<'r>>;
type PropertyMap<'r> = IndexMap<String, &'r PropertyValue>;

fn property_map_to_style<'r, 'e, E>(
    current_layer_map: &'r PropertyMap<'r>,
    base_layer_map: Option<&'r PropertyMap<'r>>,
    default_z_index: f64,
    casing_width_multiplier: f64,
    font_size_multiplier: &Option<f64>,
    osm_entity: &E,
) -> Style
where
    E: OsmEntity<'e>,
{
    let warn = |prop_map: &'r PropertyMap<'r>, prop_name, msg| {
        if let Some(val) = prop_map.get(prop_name) {
            eprintln!(
                "Entity #{}, property \"{}\" (value {:?}): {}",
                osm_entity.global_id(),
                prop_name,
                val,
                msg
            );
        }
    };

    let get_color = |prop_name| match current_layer_map.get(prop_name) {
        Some(&PropertyValue::Color(color)) => Some(color.clone()),
        Some(&PropertyValue::Identifier(id)) => {
            let color = from_color_name(id.as_str());
            if color.is_none() {
                warn(current_layer_map, prop_name, "unknown color");
            }
            color
        }
        _ => {
            warn(current_layer_map, prop_name, "expected a valid color");
            None
        }
    };

    let get_num = |prop_map: &'r PropertyMap<'r>, prop_name| match prop_map.get(prop_name) {
        Some(&PropertyValue::Numbers(nums)) if nums.len() == 1 => Some(nums[0]),
        _ => {
            warn(prop_map, prop_name, "expected a number");
            None
        }
    };

    let get_id = |prop_name| match current_layer_map.get(prop_name) {
        Some(&PropertyValue::Identifier(id)) => Some(id.as_str()),
        _ => {
            warn(current_layer_map, prop_name, "expected an identifier");
            None
        }
    };

    let get_string = |prop_name| match current_layer_map.get(prop_name) {
        Some(&PropertyValue::Identifier(id)) => Some(id.to_string()),
        Some(&PropertyValue::String(str)) => Some(str.to_string()),
        _ => {
            warn(current_layer_map, prop_name, "expected a string");
            None
        }
    };

    let get_line_cap = |prop_name| match get_id(prop_name) {
        Some("none") | Some("butt") => Some(LineCap::Butt),
        Some("round") => Some(LineCap::Round),
        Some("square") => Some(LineCap::Square),
        _ => {
            warn(current_layer_map, prop_name, "unknown line cap value");
            None
        }
    };

    let get_text_position = |prop_name| match get_id(prop_name) {
        Some("center") => Some(TextPosition::Center),
        Some("line") => Some(TextPosition::Line),
        _ => {
            warn(current_layer_map, prop_name, "unknown text position type");
            None
        }
    };

    let get_dashes = |prop_name| match current_layer_map.get(prop_name) {
        Some(&PropertyValue::Numbers(nums)) => Some(nums.clone()),
        _ => {
            warn(current_layer_map, prop_name, "expected a sequence of numbers");
            None
        }
    };

    let layer = osm_entity
        .tags()
        .get_by_key("layer")
        .and_then(|x| x.parse::<i64>().ok());
    let z_index = get_num(current_layer_map, "z-index").unwrap_or(default_z_index);

    let is_foreground_fill =
        !matches!(current_layer_map.get("fill-position"), Some(&PropertyValue::Identifier(id)) if *id == "background");

    let width = get_num(current_layer_map, "width");

    let base_width_for_casing = width
        .or_else(|| base_layer_map.and_then(|prop_map| get_num(prop_map, "width")))
        .unwrap_or_default();
    let casing_only_width = match current_layer_map.get("casing-width") {
        Some(&PropertyValue::Numbers(nums)) if nums.len() == 1 => Some(nums[0]),
        Some(&&PropertyValue::WidthDelta(num)) => Some(base_width_for_casing + num),
        _ => {
            warn(
                current_layer_map,
                "casing-width",
                "expected a number or an eval(...) statement",
            );
            None
        }
    };
    let full_casing_width = casing_only_width.map(|w| base_width_for_casing + casing_width_multiplier * w);
    let text = get_string("text");

    let font_size = get_num(current_layer_map, "font-size").map(|x| x * font_size_multiplier.unwrap_or(1.0));

    let text_style = text.map(|text| TextStyle {
        text,
        text_color: get_color("text-color"),
        text_position: get_text_position("text-position"),
        font_size,
    });

    Style {
        layer,
        z_index,

        color: get_color("color"),
        fill_color: get_color("fill-color"),
        is_foreground_fill,
        background_color: get_color("background-color"),
        opacity: get_num(current_layer_map, "opacity"),
        fill_opacity: get_num(current_layer_map, "fill-opacity"),

        width,
        dashes: get_dashes("dashes"),
        line_cap: get_line_cap("linecap"),

        casing_color: get_color("casing-color"),
        casing_width: full_casing_width,
        casing_dashes: get_dashes("casing-dashes"),
        casing_line_cap: get_line_cap("casing-linecap"),

        icon_image: get_string("icon-image"),
        fill_image: get_string("fill-image"),
        text_style,
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
                UnaryTestType::True => matches!(tag_val, Some(x) if is_true_value(x)),
                UnaryTestType::False => !matches!(tag_val, Some(x) if is_true_value(x)),
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
            let tag_val = match tags.get_by_key(tag_name).map(str::parse::<f64>) {
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
    A: StyleableEntity + OsmEntity<'e>,
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

    let good_object_type = area.matches_object_type(&selector.object_type);

    good_object_type && selector.tests.iter().all(|x| matches_by_tags(area, x))
}

fn get_layer_id(selector: &Selector) -> &str {
    match selector.layer_id {
        Some(ref id) => id,
        None => BASE_LAYER_NAME,
    }
}

const BASE_LAYER_NAME: &str = "default";

impl StyleableEntity for Node<'_> {
    fn default_z_index(&self) -> f64 {
        4.0
    }

    fn matches_object_type(&self, object_type: &ObjectType) -> bool {
        matches!(*object_type, ObjectType::Node)
    }
}

impl<A: OsmArea> StyleableEntity for A {
    fn default_z_index(&self) -> f64 {
        if self.is_closed() {
            1.0
        } else {
            3.0
        }
    }

    fn matches_object_type(&self, object_type: &ObjectType) -> bool {
        match *object_type {
            ObjectType::Way => true,
            ObjectType::Area => self.is_closed(),
            _ => false,
        }
    }
}

impl CacheableEntity for Node<'_> {
    fn cache_slot(&self) -> usize {
        0
    }
}

impl CacheableEntity for Way<'_> {
    fn cache_slot(&self) -> usize {
        if self.is_closed() {
            1
        } else {
            2
        }
    }
}

impl CacheableEntity for Multipolygon<'_> {
    fn cache_slot(&self) -> usize {
        3
    }
}
