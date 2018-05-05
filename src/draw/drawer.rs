use errors::*;

use geodata::reader::{Node, OsmEntities, OsmEntity, Relation, Way};
use mapcss::styler::{Style, Styler};
use tile as t;

use draw::TILE_SIZE;
use draw::figure::{BoundingBox, Figure};
use draw::fill::fill_contour;
use draw::icon::Icon;
use draw::line::draw_lines;
use draw::tile_pixels::{dimension, RgbTriples, RgbaColor, TilePixels};
use draw::png_writer::rgb_triples_to_png;
use draw::point::Point;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct Drawer {
    icon_cache: RwLock<HashMap<String, Option<Icon>>>,
    base_path: PathBuf,
}

#[derive(Clone, Eq, PartialEq, Hash)]
enum DrawType {
    Fill,
    Stroke,
    Casing,
}

impl Drawer {
    pub fn new(base_path: &Path) -> Drawer {
        Drawer {
            icon_cache: Default::default(),
            base_path: base_path.to_owned(),
        }
    }

    pub fn draw_tile<'a>(
        &self,
        entities: &OsmEntities<'a>,
        tile: &t::Tile,
        styler: &Styler,
    ) -> Result<Vec<u8>> {
        let pixels = self.draw_to_pixels(entities, tile, styler);
        rgb_triples_to_png(&pixels, dimension(), dimension())
    }

    pub fn draw_to_pixels<'a>(
        &self,
        entities: &OsmEntities<'a>,
        tile: &t::Tile,
        styler: &Styler,
    ) -> RgbTriples {
        let mut pixels = TilePixels::new();
        fill_canvas(&mut pixels, styler);

        let styled_ways = styler.style_entities(entities.ways.iter(), tile.zoom);

        self.draw_fills(&mut pixels, entities, tile, styler, &styled_ways);

        let draw_strokes = |draw_type, pixels: &mut TilePixels| {
            for &(way, ref style) in &styled_ways {
                self.draw_one_area(
                    pixels,
                    way,
                    style,
                    draw_type,
                    styler.use_caps_for_dashes,
                    tile,
                );
            }
        };

        draw_strokes(&DrawType::Casing, &mut pixels);
        draw_strokes(&DrawType::Stroke, &mut pixels);

        let styled_nodes = styler.style_entities(entities.nodes.iter(), tile.zoom);
        self.draw_icons(&mut pixels, tile, &styled_ways, &styled_nodes);

        pixels.to_rgb_triples()
    }

    fn draw_fills<'a>(
        &self,
        pixels: &mut TilePixels,
        entities: &OsmEntities<'a>,
        tile: &t::Tile,
        styler: &Styler,
        ways: &[(&Way<'a>, Style)],
    ) {
        let multipolygons = entities
            .relations
            .iter()
            .filter(|x| x.tags().get_by_key("type") == Some("multipolygon"));
        let styled_relations = styler.style_entities(multipolygons, tile.zoom);

        let mut rel_iter = styled_relations.iter();
        let mut way_iter = ways.iter();
        let mut rel = rel_iter.next();
        let mut way = way_iter.next();
        loop {
            let is_rel_better = {
                match (rel, way) {
                    (None, None) => break,
                    (Some(_), None) => true,
                    (None, Some(_)) => false,
                    (Some(r), Some(w)) => r.1.z_index <= w.1.z_index,
                }
            };
            if is_rel_better {
                let r = rel.unwrap();
                self.draw_one_area(
                    pixels,
                    r.0,
                    &r.1,
                    &DrawType::Fill,
                    styler.use_caps_for_dashes,
                    tile,
                );
                rel = rel_iter.next();
            } else {
                let w = way.unwrap();
                self.draw_one_area(
                    pixels,
                    w.0,
                    &w.1,
                    &DrawType::Fill,
                    styler.use_caps_for_dashes,
                    tile,
                );
                way = way_iter.next();
            }
        }
    }

    fn draw_one_area<'e, A>(
        &self,
        image: &mut TilePixels,
        area: &A,
        style: &Style,
        draw_type: &DrawType,
        use_caps_for_dashes: bool,
        tile: &t::Tile,
    ) where
        A: OsmEntity<'e> + NodePairCollection<'e>,
    {
        let mut seen_node_pairs = HashSet::new();
        let mut points = Vec::new();

        for np in area.to_node_pairs() {
            if seen_node_pairs.contains(&np) || seen_node_pairs.contains(&np.reverse()) {
                continue;
            }
            points.push(np.to_points(tile.zoom));
            seen_node_pairs.insert(np);
        }

        let create_figure = || create_figure_from_tile(tile);

        let figure = match *draw_type {
            DrawType::Fill => style.fill_color.as_ref().map(|color| {
                let mut figure = create_figure();
                fill_contour(
                    points.into_iter(),
                    color,
                    float_or_one(&style.fill_opacity),
                    &mut figure,
                );
                figure
            }),
            DrawType::Casing => style.casing_color.as_ref().and_then(|color| {
                let mut figure = create_figure();
                style.casing_width.map(|casing_width| {
                    draw_lines(
                        points.into_iter(),
                        casing_width,
                        color,
                        1.0,
                        &style.casing_dashes,
                        &style.casing_line_cap,
                        use_caps_for_dashes,
                        &mut figure,
                    );
                    figure
                })
            }),
            DrawType::Stroke => style.color.as_ref().map(|color| {
                let mut figure = create_figure();
                draw_lines(
                    points.into_iter(),
                    float_or_one(&style.width),
                    color,
                    float_or_one(&style.opacity),
                    &style.dashes,
                    &style.line_cap,
                    use_caps_for_dashes,
                    &mut figure,
                );
                figure
            }),
        };

        if let Some(ref figure) = figure {
            draw_figure(figure, image, tile);
        }
    }

    fn draw_icons(
        &self,
        image: &mut TilePixels,
        tile: &t::Tile,
        ways: &[(&Way, Style)],
        nodes: &[(&Node, Style)],
    ) {
        for &(way, ref style) in ways {
            if let Some(ref icon_image) = style.icon_image {
                if let Some((center_x, center_y)) = get_way_center(way, tile.zoom) {
                    self.draw_icon(image, tile, icon_image, center_x, center_y);
                }
            }
        }

        for &(node, ref style) in nodes {
            if let Some(ref icon_image) = style.icon_image {
                let point = Point::from_node(node, tile.zoom);
                self.draw_icon(
                    image,
                    tile,
                    icon_image,
                    f64::from(point.x),
                    f64::from(point.y),
                );
            }
        }
    }

    fn draw_icon(
        &self,
        image: &mut TilePixels,
        tile: &t::Tile,
        icon_image: &str,
        center_x: f64,
        center_y: f64,
    ) {
        {
            let read_icon_cache = self.icon_cache.read().unwrap();
            if let Some(icon) = read_icon_cache.get(icon_image) {
                if let Some(ref icon) = *icon {
                    draw_icon(image, tile, icon, center_x, center_y);
                }
                return;
            }
        }

        let full_icon_path = self.base_path.join(icon_image);
        let mut write_icon_cache = self.icon_cache.write().unwrap();
        let icon = write_icon_cache
            .entry(icon_image.to_string())
            .or_insert(match Icon::load(&full_icon_path) {
                Ok(icon) => Some(icon),
                Err(error) => {
                    let full_icon_path_str = full_icon_path.to_str().unwrap_or("N/A");
                    eprintln!("Failed to load icon from {}: {}", full_icon_path_str, error);
                    None
                }
            });
        if let Some(ref icon) = *icon {
            draw_icon(image, tile, icon, center_x, center_y);
        }
    }
}

fn fill_canvas(image: &mut TilePixels, styler: &Styler) {
    if let Some(ref c) = styler.canvas_fill_color {
        let canvas_rgba = RgbaColor::from_color(c, 1.0);
        for x in 0..TILE_SIZE {
            for y in 0..TILE_SIZE {
                image.set_pixel(x, y, &canvas_rgba);
            }
        }
    }
}

fn draw_figure(figure: &Figure, image: &mut TilePixels, tile: &t::Tile) {
    let to_tile_start = |c| (c as usize) * TILE_SIZE;
    let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));

    for (y, x_to_color) in figure
        .pixels
        .range(tile_start_y..(tile_start_y + TILE_SIZE))
    {
        let real_y = *y - tile_start_y;
        for (x, color) in x_to_color.range(tile_start_x..(tile_start_x + TILE_SIZE)) {
            let real_x = *x - tile_start_x;
            image.set_pixel(real_x, real_y, color);
        }
    }
}

fn draw_icon(image: &mut TilePixels, tile: &t::Tile, icon: &Icon, center_x: f64, center_y: f64) {
    let get_start_coord = |coord, dimension| (coord - (dimension as f64 / 2.0)) as usize;

    let start_x = get_start_coord(center_x, icon.width);
    let start_y = get_start_coord(center_y, icon.height);

    let mut figure = create_figure_from_tile(tile);
    for x in 0..icon.width {
        for y in 0..icon.height {
            figure.add(start_x + x, start_y + y, icon.get(x, y));
        }
    }
    draw_figure(&figure, image, tile);
}

fn float_or_one(num: &Option<f64>) -> f64 {
    num.unwrap_or(1.0)
}

#[derive(Eq, PartialEq, Hash)]
struct NodePair<'n> {
    n1: Node<'n>,
    n2: Node<'n>,
}

impl<'n> NodePair<'n> {
    fn to_points(&self, zoom: u8) -> (Point, Point) {
        (
            Point::from_node(&self.n1, zoom),
            Point::from_node(&self.n2, zoom),
        )
    }

    fn reverse(&self) -> NodePair<'n> {
        NodePair {
            n1: self.n2.clone(),
            n2: self.n1.clone(),
        }
    }
}

trait NodePairCollection<'a> {
    fn to_node_pairs(&self) -> Vec<NodePair<'a>>;
}

impl<'w> NodePairCollection<'w> for Way<'w> {
    fn to_node_pairs(&self) -> Vec<NodePair<'w>> {
        (1..self.node_count())
            .map(|idx| NodePair {
                n1: self.get_node(idx - 1),
                n2: self.get_node(idx),
            })
            .collect()
    }
}

impl<'r> NodePairCollection<'r> for Relation<'r> {
    fn to_node_pairs(&self) -> Vec<NodePair<'r>> {
        (0..self.way_count())
            .flat_map(|idx| self.get_way(idx).to_node_pairs())
            .collect()
    }
}

fn get_way_center(way: &Way, zoom: u8) -> Option<(f64, f64)> {
    if way.node_count() == 0 {
        return None;
    }

    let mut x = 0.0;
    let mut y = 0.0;

    for node_idx in 0..way.node_count() {
        let point = Point::from_node(&way.get_node(node_idx), zoom);
        x += f64::from(point.x);
        y += f64::from(point.y);
    }

    let norm = way.node_count() as f64;
    x /= norm;
    y /= norm;

    Some((x, y))
}

fn create_figure_from_tile(tile: &t::Tile) -> Figure {
    let to_tile_start = |c| (c as usize) * TILE_SIZE;
    let to_tile_end = |tile_start_c| tile_start_c + TILE_SIZE - 1;
    let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));
    let bounding_box = BoundingBox {
        min_x: tile_start_x,
        max_x: to_tile_end(tile_start_x),
        min_y: tile_start_y,
        max_y: to_tile_end(tile_start_y),
    };
    Figure::new(bounding_box)
}
