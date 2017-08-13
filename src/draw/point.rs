use std::cmp::{min, max};
use geodata::reader::Node;
use tile as t;

use draw::TILE_SIZE;

#[derive(Clone, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn from_node(node: &Node, tile: &t::Tile) -> Point {
        let (x, y) = t::coords_to_xy(node, tile.zoom);
        let translate = |coord, c| (coord as i32 - (c as i32 * TILE_SIZE as i32));
        Point {
            x: translate(x, tile.x),
            y: translate(y, tile.y),
        }
    }

    pub fn is_in_visible_tile(&self) -> bool {
        let is_good_coord = |c| c >= 0 && c < TILE_SIZE as i32;
        is_good_coord(self.x) && is_good_coord(self.y)
    }

    pub fn is_in_logical_tile(&self) -> bool {
        let is_good_coord = |c| c >= -(TILE_SIZE as i32) && c < 2*(TILE_SIZE as i32);
        is_good_coord(self.x) && is_good_coord(self.y)
    }

    pub fn dist_to(&self, other: &Point) -> i32 {
        (other.x - self.x).pow(2) + (other.y - self.y).pow(2)
    }

    pub fn is_between(&self, p1: &Point, p2: &Point) -> bool {
        let coord_is_between = |c, c1, c2| c >= min(c1, c2) && c <= max(c1, c2);
        coord_is_between(self.x, p1.x, p2.x) && coord_is_between(self.y, p1.y, p2.y)
    }

    pub fn clamp_by_tile(&self, other_point: &Point) -> Option<Point> {
        if self.is_in_logical_tile() {
            return Some(self.clone());
        }

        let get_coord_by_fixed_other_coord = |point_coord, point_fixed_coord, numer, denom, fixed_coord| {
            if denom == 0 {
                None
            } else {
                let result =
                    (point_coord as f64) +
                    (numer as f64 / denom as f64) * (fixed_coord - point_fixed_coord) as f64;
                Some(result.round() as i32)
            }
        };

        let dx = other_point.x - self.x;
        let dy = other_point.y - self.y;

        let get_y_by_x = |x| get_coord_by_fixed_other_coord(self.y, self.x, dy, dx, x).map(|y| Point {x, y});
        let get_x_by_y = |y| get_coord_by_fixed_other_coord(self.x, self.y, dx, dy, y).map(|x| Point {x, y});

        let first_valid_coord = -(TILE_SIZE as i32);
        let last_valid_coord = (2 * TILE_SIZE - 1) as i32;
        let intersections_with_tile = [
            get_x_by_y(first_valid_coord),
            get_x_by_y(last_valid_coord),
            get_y_by_x(first_valid_coord),
            get_y_by_x(last_valid_coord),
        ];

        intersections_with_tile.into_iter()
            .filter_map(|x| x.clone())
            .filter(|x| x.is_in_logical_tile() && x.is_between(self, other_point))
            .min_by_key(|x| x.dist_to(self))
    }
}
