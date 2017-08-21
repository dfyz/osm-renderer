use errors::*;

use geodata::reader::{OsmEntities, OsmEntity, Way};
use mapcss::styler::{Style, Styler};
use tile as t;

use draw::TILE_SIZE;
use draw::drawer::Drawer;
use draw::figure::Figure;
use draw::line::draw_thick_line;
use draw::png_image::{PngImage, RgbaColor};
use draw::point::Point;

use std::collections::HashMap;
use std::sync::RwLock;

pub struct PureRustDrawer {
    cache: RwLock<HashMap<CacheKey, Figure>>,
}

#[derive(Eq, PartialEq, Hash)]
struct CacheKey {
    entity_id: u64,
    style: Style,
    zoom_level: u8,
}

impl PureRustDrawer {
    pub fn new() -> PureRustDrawer {
        PureRustDrawer {
            cache: Default::default(),
        }
    }

    fn draw_ways(&self, image: &mut PngImage, styled_ways: Vec<(&Way, Style)>, tile: &t::Tile) {
        let ways_to_draw = styled_ways.into_iter()
            .filter(|&(w, _)| {
                w.node_count() > 0
            });

        for (way, ref style) in ways_to_draw {
            let cache_key = CacheKey {
                entity_id: way.global_id(),
                style: (*style).clone(),
                zoom_level: tile.zoom,
            };

            {
                let read_cache = self.cache.read().unwrap();
                if let Some(ref figure) = read_cache.get(&cache_key) {
                    draw_figure(figure, image, tile);
                    continue;
                }
            }

            let figure = way_to_figure(way, style, tile.zoom);
            draw_figure(&figure, image, tile);
            let mut write_cache = self.cache.write().unwrap();
            write_cache.insert(cache_key, figure);
        }
    }
}

impl Drawer for PureRustDrawer {
    fn draw_tile<'a>(&self, entities: &OsmEntities<'a>, tile: &t::Tile, styler: &Styler) -> Result<Vec<u8>> {
        let mut image = PngImage::new();
        fill_canvas(&mut image, styler);

        let styled_ways = styler.style_ways(entities.ways.iter(), tile.zoom);
        self.draw_ways(&mut image, styled_ways, tile);

        image.to_bytes()
    }
}

fn fill_canvas(image: &mut PngImage, styler: &Styler) {
    if let Some(ref c) = styler.canvas_fill_color {
        let canvas_rgba = RgbaColor::from_color(c, 1.0);
        for x in 0..TILE_SIZE {
            for y in 0..TILE_SIZE {
                image.set_pixel(x, y, &canvas_rgba);
            }
        }
    }
}

fn way_to_figure(way: &Way, style: &Style, zoom: u8) -> Figure {
    let mut figure: Figure = Default::default();
    if let Some(ref color) = style.color {
        for i in 1..way.node_count() {
            let p1 = Point::from_node(&way.get_node(i - 1), zoom);
            let p2 = Point::from_node(&way.get_node(i), zoom);

            let width = style.width.unwrap_or(1.0).into();
            let opacity = style.opacity.unwrap_or(1.0).into();
            draw_thick_line(&p1, &p2, width, color, opacity, &mut figure);
        }
    }
    figure
}

fn draw_figure(figure: &Figure, image: &mut PngImage, tile: &t::Tile) {
    let to_tile_start = |c| (c as usize) * TILE_SIZE;
    let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));

    for (y, x_to_color) in figure.pixels.range(tile_start_y..(tile_start_y + TILE_SIZE)) {
        let real_y = *y - tile_start_y;
        for (x, color) in x_to_color.range(tile_start_x..(tile_start_x + TILE_SIZE)) {
            let real_x = *x - tile_start_x;
            image.set_pixel(real_x, real_y, color);
        }
    }
}
