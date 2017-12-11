use geodata::reader::Node;
use tile as t;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn from_node(node: &Node, zoom: u8) -> Point {
        let (x, y) = t::coords_to_xy(node, zoom);
        Point { x: x as i32, y: y as i32 }
    }

    pub fn dist(&self, other: &Point) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (f64::from(dx * dx) + f64::from(dy * dy)).sqrt()
    }

    pub fn push_away_from(&self, other: &Point, by: f64) -> Point {
        let dist = self.dist(&other);
        let push_away_dist = by / dist;
        let push_away_coord = |our_c, other_c| our_c + (f64::from(our_c - other_c) * push_away_dist).round() as i32;
        Point {
            x: push_away_coord(self.x, other.x),
            y: push_away_coord(self.y, other.y),
        }
    }
}
