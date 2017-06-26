use errors::*;

use geodata::reader::{OsmEntities, Way};
use mapcss::styler::{Style, Styler};
use tile as t;

use drawer::TILE_SIZE;
use drawer::line::draw_thick_line;
use drawer::png_image::{PngImage, RgbaColor};
use drawer::point::Point;

use std::collections::HashMap;
use std::collections::hash_map::Entry;

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &t::Tile, styler: &Styler) -> Result<Vec<u8>> {
    let mut image = PngImage::new();
    fill_canvas(&mut image, styler);

    let styled_ways = styler.style_ways(entities.ways.iter(), tile.zoom);
    draw_ways(&mut image, styled_ways, tile);

    image.to_bytes()
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

fn draw_ways(image: &mut PngImage, styled_ways: Vec<(&Way, Style)>, tile: &t::Tile) {
    let ways_to_draw = styled_ways.into_iter()
        .filter(|&(w, _)| {
            w.node_count() > 0
        });

    for (way, ref style) in ways_to_draw {
        if let Some(ref color) = style.color {
            let mut pixels: HashMap<(usize, usize), f64> = HashMap::new();

            for i in 1..way.node_count() {
                let set_pixel = |x, y, opacity| {
                    match pixels.entry((x, y)) {
                        Entry::Occupied(o) => {
                            *o.into_mut() = o.get().max(opacity);
                        },
                        Entry::Vacant(v) => {
                            v.insert(opacity);
                        },
                    }
                };

                let p1 = Point::from_node(&way.get_node(i - 1), tile);
                let p2 = Point::from_node(&way.get_node(i), tile);


                if let (Some(clamped_p1), Some(clamped_p2)) = (p1.clamp_by_tile(&p2), p2.clamp_by_tile(&p1)) {
                    let width = style.width.unwrap_or(1.0);
                    let opacity = style.opacity.unwrap_or(1.0);
                    draw_thick_line(&clamped_p1, &clamped_p2, width, opacity, set_pixel);
                }
            }

            for (k, v) in pixels.iter() {
                image.set_pixel(k.0, k.1, &RgbaColor::from_color(color, *v));
            }
        }
    }
}
