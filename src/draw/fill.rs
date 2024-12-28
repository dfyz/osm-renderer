use crate::draw::icon::Icon;
use crate::draw::point::Point;
use crate::draw::point_pairs::PointPairIter;
use crate::draw::tile_pixels::RgbaColor;
use crate::mapcss::color::Color;

use crate::draw::tile_pixels::TilePixels;
use indexmap::IndexMap;
use std::cmp::{max, min};

pub enum Filler<'a> {
    Color(&'a Color),
    Image(&'a Icon),
}

pub fn fill_contour(points: PointPairIter<'_>, filler: &Filler<'_>, opacity: f64, pixels: &mut TilePixels) {
    let mut y_to_edges = EdgesByY::default();

    for (idx, (p1, p2)) in points.enumerate() {
        draw_line(idx, &p1, &p2, &mut y_to_edges, pixels.bb().min_y, pixels.bb().max_y);
    }

    for (y, edges) in y_to_edges.iter() {
        let mut good_edges = edges.values().filter(|e| !e.is_poisoned).collect::<Vec<_>>();
        good_edges.sort_by_key(|e| e.x_min);

        let mut idx = 0;
        while idx + 1 < good_edges.len() {
            let e1 = good_edges[idx];
            let e2 = good_edges[idx + 1];
            let from_x = e1.x_min.max(pixels.bb().min_x);
            let to_x = e2.x_max.min(pixels.bb().max_x) + 1;
            for x in from_x..to_x {
                let fill_color = match filler {
                    Filler::Color(color) => RgbaColor::from_color(color, opacity),
                    Filler::Image(icon) => {
                        let icon_x = (x as usize) % icon.width;
                        let icon_y = (*y as usize) % icon.height;
                        icon.get(icon_x, icon_y)
                    }
                };
                pixels.set_pixel(x, *y, &fill_color);
            }
            idx += 2;
        }
    }
}

// Stripped-down version of Bresenham which is extremely easy to implement.
// See http://members.chello.at/~easyfilter/bresenham.html
fn draw_line(edge_idx: usize, p1: &Point, p2: &Point, y_to_edges: &mut EdgesByY, min_y: i32, max_y: i32) {
    let dx = (p2.x - p1.x).abs();
    let dy = -(p2.y - p1.y).abs();

    let get_dir = |c1, c2| if c1 < c2 { 1 } else { -1 };
    let sx = get_dir(p1.x, p2.x);
    let sy = get_dir(p1.y, p2.y);

    let mut err = dx + dy;
    let mut cur_point = p1.clone();

    loop {
        let is_start = cur_point == *p1;
        let is_end = cur_point == *p2;

        let is_poisoned = if is_start {
            p1.y <= p2.y
        } else if is_end {
            p2.y <= p1.y
        } else {
            false
        };

        if cur_point.y >= min_y && cur_point.y <= max_y {
            let edge = y_to_edges
                .entry(cur_point.y)
                .or_default()
                .entry(edge_idx)
                .or_insert_with(|| Edge {
                    x_min: cur_point.x,
                    x_max: cur_point.x,
                    is_poisoned,
                });

            edge.x_min = min(edge.x_min, cur_point.x);
            edge.x_max = max(edge.x_max, cur_point.x);
            edge.is_poisoned |= is_poisoned;
        }

        if is_end {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            cur_point.x += sx;
        }
        if e2 <= dx {
            err += dx;
            cur_point.y += sy;
        }
    }
}

type EdgesByY = IndexMap<i32, IndexMap<usize, Edge>>;

struct Edge {
    x_min: i32,
    x_max: i32,
    is_poisoned: bool,
}
