use crate::draw::point::Point;
use crate::geodata::reader::{Multipolygon, Polygon, Way};

pub type PointPairIter<'a> = Box<dyn Iterator<Item = (Point, Point)> + 'a>;

pub trait PointPairCollection<'a> {
    fn to_point_pairs(&'a self, zoom: u8) -> PointPairIter<'a>;
}

macro_rules! implement_to_point_pairs {
    ($s:expr, $zoom:expr) => {
        Box::new((1..$s.node_count()).map(move |idx| {
            let n1 = $s.get_node(idx - 1);
            let n2 = $s.get_node(idx);
            (Point::from_node(&n1, $zoom), Point::from_node(&n2, $zoom))
        }))
    };
}

impl<'w> PointPairCollection<'w> for Way<'w> {
    fn to_point_pairs(&'w self, zoom: u8) -> PointPairIter<'w> {
        implement_to_point_pairs!(self, zoom)
    }
}

impl<'p> Polygon<'p> {
    fn into_point_pairs(self, zoom: u8) -> PointPairIter<'p> {
        implement_to_point_pairs!(self, zoom)
    }
}

impl<'r> PointPairCollection<'r> for Multipolygon<'r> {
    fn to_point_pairs(&'r self, zoom: u8) -> PointPairIter<'r> {
        let polygon_count = self.polygon_count();
        Box::new((0..polygon_count).flat_map(move |idx| self.get_polygon(idx).into_point_pairs(zoom)))
    }
}
