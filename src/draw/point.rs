use geodata::reader::Node;
use tile as t;

#[derive(Clone, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn from_node(node: &Node, zoom: u8) -> Point {
        let (x, y) = t::coords_to_xy(node, zoom);
        Point { x: x as i32, y: y as i32 }
    }
}
