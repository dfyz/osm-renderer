use errors::*;

use geodata::reader::{OsmEntities, OsmEntity, Node, Relation, Way};
use mapcss::styler::{Style, StyleHashKey, Styler};
use tile as t;

use draw::TILE_SIZE;
use draw::figure::Figure;
use draw::fill::fill_contour;
use draw::line::draw_lines;
use draw::tile_pixels::{dimension, RgbTriples, RgbaColor, TilePixels};
use draw::png_writer::rgb_triples_to_png;
use draw::point::Point;

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

#[derive(Default)]
pub struct Drawer {
    cache: RwLock<HashMap<CacheKey, Figure>>,
}

#[derive(Eq, PartialEq, Hash)]
struct CacheKey {
    entity_id: u64,
    style: StyleHashKey,
    zoom_level: u8,
    is_fill: bool,
}

impl Drawer {
    pub fn new() -> Drawer {
        Drawer {
            cache: Default::default(),
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

        let styled_ways = styler.style_areas(entities.ways.iter(), tile.zoom);

        let multipolygons = entities
            .relations
            .iter()
            .filter(|x| x.tags().get_by_key("type") == Some("multipolygon"));
        let styled_relations = styler.style_areas(multipolygons, tile.zoom);

        for &(way, ref style) in styled_ways.iter() {
            self.draw_one_area(&mut pixels, way, style, true, tile);
        }

        for &(rel, ref style) in styled_relations.iter() {
            self.draw_one_area(&mut pixels, rel, style, true, tile);
        }

        for &(way, ref style) in styled_ways.iter() {
            self.draw_one_area(&mut pixels, way, style, false, tile);
        }

        pixels.to_rgb_triples()
    }

    fn draw_one_area<'e, A>(
        &self,
        image: &mut TilePixels,
        area: &A,
        style: &Style,
        is_fill: bool,
        tile: &t::Tile,
    ) where
        A: OsmEntity<'e> + NodePairCollection<'e>,
    {
        let cache_key = CacheKey {
            entity_id: area.global_id(),
            style: style.to_hash_key(),
            zoom_level: tile.zoom,
            is_fill,
        };

        {
            let read_cache = self.cache.read().unwrap();
            if let Some(figure) = read_cache.get(&cache_key) {
                draw_figure(figure, image, tile);
                return;
            }
        }

        let mut seen_node_pairs = HashSet::new();
        let mut points = Vec::new();

        for np in area.to_node_pairs() {
            if seen_node_pairs.contains(&np) || seen_node_pairs.contains(&np.reverse()) {
                continue;
            }
            points.push(np.to_points(tile.zoom));
            seen_node_pairs.insert(np);
        }

        let figure = if is_fill {
            style
                .fill_color
                .as_ref()
                .map(|color| fill_contour(points.into_iter(), color, float_or_one(&style.fill_opacity)))
        } else {
            style.color.as_ref().map(|color| {
                draw_lines(
                    points.into_iter(),
                    float_or_one(&style.width),
                    color,
                    float_or_one(&style.opacity),
                    &style.dashes,
                    &style.line_cap,
                )
            })
        };

        if let Some(ref figure) = figure {
            draw_figure(figure, image, tile);
        }
        let mut write_cache = self.cache.write().unwrap();
        write_cache.insert(cache_key, figure.unwrap_or_default());
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
            .map(|idx|
                NodePair {
                    n1: self.get_node(idx - 1),
                    n2: self.get_node(idx)
                }
            )
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
