use errors::*;

use geodata::reader::{Node, OsmEntities, Way};
use mapcss::color::Color;
use mapcss::styler::{Style, Styler};
use png::{ColorType, Encoder, HasParameters};
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
                let mut p1 = Point::from_node(&way.get_node(i - 1), tile);
                let p2 = Point::from_node(&way.get_node(i), tile);

                if !p1.is_in_tile() {
                    match clamp_by_tile(&p1, &p2) {
                        Some(ref new_p1) if p1.dist_to(new_p1) < p1.dist_to(&p2) => {
                            p1 = new_p1.clone();
                        },
                        _ => continue,
                    }
                }

                draw_segment(image, &p1, &p2, c);
            }
        }
    }
}

fn clamp_by_tile(p1: &Point, p2: &Point) -> Option<Point> {
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
        .filter(|x| x.is_in_tile())
        .min_by_key(|x| x.dist_to(p1))
}

fn draw_segment(image: &mut PngImage, p1: &Point, p2: &Point, color: &Color) {
    let mut cur_x = p1.x;
    let mut cur_y = p1.y;

    let get_error = |x: i32, y: i32| {
        ((y - p1.y) * (p2.x - p1.x) - (x - p1.x) * (p2.y - p1.y)).abs()
    };

    let dx = if p1.x <= p2.x { 1 } else { -1 };
    let dy = if p1.y <= p2.y { 1 } else { -1 };

    let reached_end = |from, to, dir| dir * from >= dir * to;

    while !reached_end(cur_x, p2.x, dx) || !reached_end(cur_y, p2.y, dy) {
        let cur_point = Point {
            x: cur_x,
            y: cur_y
        };
        if !cur_point.is_in_tile() {
            break;
        }
        image.set_pixel(cur_x as usize, cur_y as usize, color);
        let err_xy = get_error(cur_x + dx, cur_y + dy);
        let should_move_x = err_xy <= get_error(cur_x, cur_y + dy);
        let should_move_y = err_xy <= get_error(cur_x + dx, cur_y);

        if should_move_x {
            cur_x += dx;
        }
        if should_move_y {
            cur_y += dy;
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
