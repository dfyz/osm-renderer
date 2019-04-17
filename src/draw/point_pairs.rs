use crate::draw::point::Point;
use crate::geodata::reader::{Multipolygon, Polygon, Way};
use crate::tile::Tile;

pub type PointPairIter<'a> = Box<dyn Iterator<Item = (Point, Point)> + 'a>;

pub trait PointPairCollection<'a> {
    fn to_point_pairs(&'a self, tile: &'a Tile, scale: f64) -> PointPairIter<'a>;
}

macro_rules! implement_to_point_pairs {
    ($s:expr, $tile:expr, $scale:expr) => {
        Box::new((1..$s.node_count()).map(move |idx| {
            let n1 = $s.get_node(idx - 1);
            let n2 = $s.get_node(idx);
            (
                Point::from_node(&n1, $tile, $scale),
                Point::from_node(&n2, $tile, $scale),
            )
        }))
    };
}

impl<'w> PointPairCollection<'w> for Way<'w> {
    fn to_point_pairs(&'w self, tile: &'w Tile, scale: f64) -> PointPairIter<'w> {
        implement_to_point_pairs!(self, tile, scale)
    }
}

impl<'p> Polygon<'p> {
    fn into_point_pairs(self, tile: &'p Tile, scale: f64) -> PointPairIter<'p> {
        implement_to_point_pairs!(self, tile, scale)
    }
}

impl<'r> PointPairCollection<'r> for Multipolygon<'r> {
    fn to_point_pairs(&'r self, tile: &'r Tile, scale: f64) -> PointPairIter<'r> {
        let polygon_count = self.polygon_count();
        Box::new((0..polygon_count).flat_map(move |idx| self.get_polygon(idx).into_point_pairs(tile, scale)))
    }
}
