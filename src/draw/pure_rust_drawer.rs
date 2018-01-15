use errors::*;

use geodata::reader::{OsmEntities, OsmEntity, Way};
use mapcss::styler::{Style, StyleHashKey, Styler};
use tile as t;

use draw::TILE_SIZE;
use draw::drawer::Drawer;
use draw::figure::Figure;
use draw::fill::fill_contour;
use draw::line::draw_lines;
use draw::tile_pixels::{dimension, RgbTriples, RgbaColor, TilePixels};
use draw::png_writer::rgb_triples_to_png;
use draw::point::Point;

use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Default)]
pub struct PureRustDrawer {
    cache: RwLock<HashMap<CacheKey, Figure>>,
}

#[derive(Eq, PartialEq, Hash)]
struct CacheKey {
    entity_id: u64,
    style: StyleHashKey,
    zoom_level: u8,
    is_fill: bool,
}

impl PureRustDrawer {
    pub fn new() -> PureRustDrawer {
        PureRustDrawer {
            cache: Default::default(),
        }
    }

    pub fn draw_to_pixels<'a>(
        &self,
        entities: &OsmEntities<'a>,
        tile: &t::Tile,
        styler: &Styler,
    ) -> RgbTriples {
        let mut pixels = TilePixels::new();
        fill_canvas(&mut pixels, styler);

        let styled_ways = styler.style_ways(entities.ways.iter(), tile.zoom);
        self.draw_ways(&mut pixels, &styled_ways, tile);

        pixels.to_rgb_triples()
    }

    fn draw_ways(&self, image: &mut TilePixels, styled_ways: &[(&Way, Style)], tile: &t::Tile) {
        let ways_to_draw = || styled_ways.iter().filter(|&&(w, _)| w.node_count() > 0);

        for &(way, ref style) in ways_to_draw() {
            self.draw_one_way(image, way, style, true, tile);
        }

        for &(way, ref style) in ways_to_draw() {
            self.draw_one_way(image, way, style, false, tile);
        }
    }

    fn draw_one_way(
        &self,
        image: &mut TilePixels,
        way: &Way,
        style: &Style,
        is_fill: bool,
        tile: &t::Tile,
    ) {
        let cache_key = CacheKey {
            entity_id: way.global_id(),
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

        let points = (1..way.node_count()).map(|idx| {
            let p1 = Point::from_node(&way.get_node(idx - 1), tile.zoom);
            let p2 = Point::from_node(&way.get_node(idx), tile.zoom);
            (p1, p2)
        });

        let figure = if is_fill {
            style
                .fill_color
                .as_ref()
                .map(|color| fill_contour(points, color, float_or_one(&style.fill_opacity)))
        } else {
            style.color.as_ref().map(|color| {
                draw_lines(
                    points,
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

impl Drawer for PureRustDrawer {
    fn draw_tile<'a>(
        &self,
        entities: &OsmEntities<'a>,
        tile: &t::Tile,
        styler: &Styler,
    ) -> Result<Vec<u8>> {
        let pixels = self.draw_to_pixels(entities, tile, styler);
        rgb_triples_to_png(&pixels, dimension(), dimension())
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
