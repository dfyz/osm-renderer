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
        if let Some(ref c) = style.color {
            for i in 1..way.node_count() {
                let p1 = Point::from_node(&way.get_node(i - 1), tile);
                let p2 = Point::from_node(&way.get_node(i), tile);

                match (clamp_by_tile(&p1, &p2), clamp_by_tile(&p2, &p1)) {
                    (Some(clamped_p1), Some(clamped_p2)) => {
                        let width = style.width.unwrap_or(1.0);
                        let opacity = style.opacity.unwrap_or(1.0);
                        draw_thick_line(image, &clamped_p1, &clamped_p2, c, width, opacity);
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

fn swap_x_y_if_needed<T>(a: T, b: T, should_swap: bool) -> (T, T) {
    if should_swap {
        (b, a)
    } else {
        (a, b)
    }
}

fn draw_thick_line(image: &mut PngImage, p1: &Point, p2: &Point, color: &Color, width: f64, opacity: f64) {
    let get_inc = |from, to| if from <= to { 1 } else { -1 };

    let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
    let (mut x0, mut y0) = (p1.x, p1.y);
    let should_swap_x_y = dx > dy;

    let (mn, mx) = swap_x_y_if_needed(&mut x0, &mut y0, should_swap_x_y);
    let (mn_last, mx_last) = swap_x_y_if_needed(p2.x, p2.y, should_swap_x_y);
    let (mn_delta, mx_delta) = swap_x_y_if_needed(dx, dy, should_swap_x_y);
    let (mn_inc, mx_inc) = swap_x_y_if_needed(get_inc(p1.x, p2.x), get_inc(p1.y, p2.y), should_swap_x_y);

    let mut error = 0;
    let mut p_error = 0;

    let update_error = |error: &mut i32| {
        let mut was_corrected = false;
        if *error + 2 * mn_delta > mx_delta {
            *error -= 2 * mx_delta;
            was_corrected = true;
        }
        *error += 2 * mn_delta;
        was_corrected
    };

    let line_dist_numer_const = ((p2.x * p1.y) - (p2.y * p1.x)) as f64;
    let line_dist_denom = ((dy*dy + dx*dx) as f64).sqrt();
    let half_width = width / 2.0;
    let feather_from = half_width - 0.5;
    let feather_to = half_width + 0.5;
    let opacity_mul = opacity * width.min(1.0);

    let is_in_tile = |x, y| Point{x, y}.is_in_tile();

    let draw_perpendiculars = |image: &mut PngImage, mn, mx, p_error| {
        let mut draw_one_perpendicular = |mul| {
            let mut p_mn = mx;
            let mut p_mx = mn;
            let mut error = mul * p_error;
            loop {
                let (perp_x, perp_y) = swap_x_y_if_needed(p_mx, p_mn, should_swap_x_y);

                let line_dist_numer_non_const = ((p2.y - p1.y) * perp_x - (p2.x - p1.x) * perp_y) as f64;
                let line_dist = (line_dist_numer_const + line_dist_numer_non_const).abs() / line_dist_denom;

                let opacity = if line_dist < feather_from {
                    opacity_mul
                } else if line_dist < feather_to {
                    (feather_to - line_dist) * opacity_mul
                } else {
                    break;
                };

                if !is_in_tile(perp_x, perp_y) {
                    break;
                }

                image.set_pixel_with_opacity(perp_x as usize, perp_y as usize, &RgbaColor::from_color(color, opacity));

                if update_error(&mut error) {
                    p_mn -= mul * mx_inc;
                }
                p_mx += mul * mn_inc;
            }
        };

        draw_one_perpendicular(1);
        draw_one_perpendicular(-1);
    };

    loop {
        draw_perpendiculars(image, *mn, *mx, p_error);

        if *mn == mn_last && *mx == mx_last {
            break;
        }

        if update_error(&mut error) {
            *mn += mn_inc;
            if update_error(&mut p_error) {
                draw_perpendiculars(image, *mn, *mx, p_error);
            }
        }
        *mx += mx_inc;
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

#[derive(Clone)]
struct RgbaColor {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl RgbaColor {
    fn from_color(color: &Color, opacity: f64) -> RgbaColor {
        let premultiply = |c| opacity * ((c as f64) / (u8::max_value() as f64));

        RgbaColor {
            r: premultiply(color.r),
            g: premultiply(color.g),
            b: premultiply(color.b),
            a: opacity,
        }
    }
}

struct PngImage {
    pixels: Vec<RgbaColor>,
}

impl PngImage {
    fn new() -> PngImage {
        PngImage {
            pixels: vec![
                RgbaColor { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
                TOTAL_PIXELS
            ],
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: &RgbaColor) {
        self.pixels[to_idx(x, y)] = color.clone();
    }

    fn set_pixel_with_opacity(&mut self, x: usize, y: usize, color: &RgbaColor) {
        let idx = to_idx(x, y);
        let new_pixel = {
            let ref old_pixel = self.pixels[idx];
            let blend = |new_value, old_value| new_value + (1.0 - color.a) * old_value;
            RgbaColor {
                r: blend(color.r, old_pixel.r),
                g: blend(color.g, old_pixel.g),
                b: blend(color.b, old_pixel.b),
                a: blend(color.a, old_pixel.a),
            }
        };
        self.pixels[idx] = new_pixel;
    }

    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        {
            let mut png_encoder = Encoder::new(&mut buf, t::TILE_SIZE, t::TILE_SIZE);
            png_encoder.set(ColorType::RGB);
            let mut png_writer = png_encoder.write_header().chain_err(|| "Failed to write PNG header")?;

            let mut image_bytes = Vec::new();
            for p in &self.pixels {
                let postdivide = |val| {
                    let mul = if p.a == 0.0 { 0.0 } else { val / p.a };
                    ((u8::max_value() as f64) * mul) as u8
                };
                image_bytes.extend([
                    postdivide(p.r),
                    postdivide(p.g),
                    postdivide(p.b),
                ].into_iter());
            }
            png_writer.write_image_data(image_bytes.as_slice()).chain_err(|| "Failed to write PNG data")?;
        }
        Ok(buf)
    }
}

fn to_idx(x: usize, y: usize) -> usize {
    (y * TILE_SIZE) + x
}