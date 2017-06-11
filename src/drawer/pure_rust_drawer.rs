use errors::*;

use geodata::reader::{Node, OsmEntities, Way};
use mapcss::color::Color;
use mapcss::styler::{Style, Styler};
use png::{ColorType, Encoder, HasParameters};
use std::cmp::{min, max};
use tile as t;

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &t::Tile, styler: &Styler) -> Result<Vec<u8>> {
    let mut image = PngImage::new();
    fill_canvas(&mut image, styler);

    let styled_ways = styler.style_ways(entities.ways.iter(), tile.zoom);
    draw_ways(&mut image, styled_ways, tile);

    image.to_bytes()
}

fn fill_canvas(image: &mut PngImage, styler: &Styler) {
    if let Some(ref c) = styler.canvas_fill_color {
        for x in 0..TILE_SIZE {
            for y in 0..TILE_SIZE {
                image.set_pixel(x, y, c);
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
        if let Some(ref c) = style.color {
            for i in 1..way.node_count() {
                let p1 = Point::from_node(&way.get_node(i - 1), tile);
                let p2 = Point::from_node(&way.get_node(i), tile);

                match (clamp_by_tile(&p1, &p2), clamp_by_tile(&p2, &p1)) {
                    (Some(clamped_p1), Some(clamped_p2)) => {
                        draw_thick_line(image, &clamped_p1, &clamped_p2, c, style.width.unwrap_or(1.0));
                    },
                    _ => {},
                }
            }
        }
    }
}

fn clamp_by_tile(p1: &Point, p2: &Point) -> Option<Point> {
    if p1.is_in_tile() {
        return Some(p1.clone());
    }

    let get_coord_by_fixed_other_coord = |p1_coord, p1_fixed_coord, numer, denom, fixed_coord| {
        if denom == 0 {
            None
        } else {
            let result =
                (p1_coord as f64) +
                (numer as f64 / denom as f64) * (fixed_coord - p1_fixed_coord) as f64;
            Some(result.round() as i32)
        }
    };

    let dx = p2.x - p1.x;
    let dy = p2.y - p1.y;

    let get_y_by_x = |x| get_coord_by_fixed_other_coord(p1.y, p1.x, dy, dx, x).map(|y| Point {x, y});
    let get_x_by_y = |y| get_coord_by_fixed_other_coord(p1.x, p1.y, dx, dy, y).map(|x| Point {x, y});

    let last_valid_coord = (TILE_SIZE - 1) as i32;
    let intersections_with_tile = [
        get_x_by_y(0),
        get_x_by_y(last_valid_coord),
        get_y_by_x(0),
        get_y_by_x(last_valid_coord),
    ];

    intersections_with_tile.into_iter()
        .filter_map(|x| x.clone())
        .filter(|x| x.is_in_tile() && x.is_between(p1, p2))
        .min_by_key(|x| x.dist_to(p1))
}

fn draw_thick_line(image: &mut PngImage, p1: &Point, p2: &Point, color: &Color, width: f64) {
    let reached_end = |from, to, dir| dir * from >= dir * to;
    let should_stop = |point: &Point, dx, dy| reached_end(point.x, p2.x, dx) && reached_end(point.y, p2.y, dy);

    draw_line(image, p1, p2, color, width, &should_stop);
}

fn draw_line(image: &mut PngImage, p1: &Point, p2: &Point, color: &Color, width: f64, should_stop: &Fn(&Point, i32, i32) -> bool) {
    let get_error = |x: i32, y: i32| {
        ((y - p1.y) * (p2.x - p1.x) - (x - p1.x) * (p2.y - p1.y)).abs()
    };

    let dx = if p1.x <= p2.x { 1 } else { -1 };
    let dy = if p1.y <= p2.y { 1 } else { -1 };

    let mut cur_point = Point {
        x: p1.x,
        y: p1.y,
    };

    let get_perpendicular = |point_from: &Point, sign| Point {
        x: point_from.x + sign * (p2.y - p1.y),
        y: point_from.y - sign * (p2.x - p1.x),
    };

    while !should_stop(&cur_point, dx, dy) && cur_point.is_in_tile() {
        image.set_pixel(cur_point.x as usize, cur_point.y as usize, color);

        if width > 1.0 {
            let should_stop_perpendicular = |p: &Point, _, _| {
                ((4 * p.dist_to(&cur_point)) as f64) > width.powi(2)
            };

            draw_line(image, &cur_point, &get_perpendicular(&cur_point, -1), color, 1.0, &should_stop_perpendicular);
            draw_line(image, &cur_point, &get_perpendicular(&cur_point, 1), color, 1.0, &should_stop_perpendicular);
        }

        let err_xy = get_error(cur_point.x + dx, cur_point.y + dy);
        let should_move_x = err_xy <= get_error(cur_point.x, cur_point.y + dy);
        let should_move_y = err_xy <= get_error(cur_point.x + dx, cur_point.y);

        if should_move_x {
            cur_point.x += dx;
        }
        if should_move_y {
            cur_point.y += dy;
        }
    }
}

const TILE_SIZE: usize = t::TILE_SIZE as usize;
const TOTAL_PIXELS: usize = TILE_SIZE * TILE_SIZE;

#[derive(Clone, Debug)]
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn from_node(node: &Node, tile: &t::Tile) -> Point {
        let (x, y) = t::coords_to_xy(node, tile.zoom);
        let translate = |coord, c| (coord as i32 - (c as i32 * TILE_SIZE as i32));
        Point {
            x: translate(x, tile.x),
            y: translate(y, tile.y),
        }
    }

    fn is_in_tile(&self) -> bool {
        let is_good_coord = |c| c >= 0 && c < TILE_SIZE as i32;
        is_good_coord(self.x) && is_good_coord(self.y)
    }

    fn dist_to(&self, other: &Point) -> i32 {
        (other.x - self.x).pow(2) + (other.y - self.y).pow(2)
    }

    fn is_between(&self, p1: &Point, p2: &Point) -> bool {
        let coord_is_between = |c, c1, c2| c >= min(c1, c2) && c <= max(c1, c2);
        coord_is_between(self.x, p1.x, p2.x) && coord_is_between(self.y, p1.y, p2.y)
    }
}

struct PngImage {
    pixels: Vec<Color>,
}

impl PngImage {
    fn new() -> PngImage {
        PngImage {
            pixels: vec![
                Color { r: 0, g: 0, b: 0 };
                TOTAL_PIXELS
            ],
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: &Color) {
        self.pixels[(y * TILE_SIZE) + x] = color.clone();
    }

    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        {
            let mut png_encoder = Encoder::new(&mut buf, t::TILE_SIZE, t::TILE_SIZE);
            png_encoder.set(ColorType::RGB);
            let mut png_writer = png_encoder.write_header().chain_err(|| "Failed to write PNG header")?;

            let mut image_bytes = Vec::new();
            for p in &self.pixels {
                image_bytes.extend([p.r, p.g, p.b].into_iter());
            }
            png_writer.write_image_data(image_bytes.as_slice()).chain_err(|| "Failed to write PNG data")?;
        }
        Ok(buf)
    }
}
