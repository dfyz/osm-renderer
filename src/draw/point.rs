use crate::geodata::reader::Node;
use crate::tile as t;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn from_node(node: &Node<'_>, zoom: u8, scale: f64) -> Point {
        let (x, y) = t::coords_to_xy(node, zoom);
        let to_coord = |c| (c * scale) as i32;
        Point {
            x: to_coord(x),
            y: to_coord(y),
        }
    }

    pub fn dist(&self, other: &Point) -> f64 {
        let dx = f64::from(self.x - other.x);
        let dy = f64::from(self.y - other.y);
        (dx * dx + dy * dy).sqrt()
    }

    pub fn push_away_from(&self, other: &Point, by: f64) -> Point {
        let dist = self.dist(other);
        let push_away_dist = by / dist;
        let push_away_coord = |our_c, other_c| our_c + (f64::from(our_c - other_c) * push_away_dist).round() as i32;
        Point {
            x: push_away_coord(self.x, other.x),
            y: push_away_coord(self.y, other.y),
        }
    }
}
