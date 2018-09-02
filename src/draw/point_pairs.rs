use draw::point::Point;
use geodata::reader::{Multipolygon, Polygon, Way};

pub type PointPairs = Vec<(Point, Point)>;

pub trait PointPairCollection {
    fn to_point_pairs(&self, zoom: u8) -> PointPairs;
}

macro_rules! implement_to_point_pairs {
    () => {
        fn to_point_pairs(&self, zoom: u8) -> PointPairs {
            (1..self.node_count())
                .map(|idx| (
                    Point::from_node(&self.get_node(idx - 1), zoom),
                    Point::from_node(&self.get_node(idx), zoom),
                ))
                .collect()
        }
    }
}

impl<'w> PointPairCollection for Way<'w> {
    implement_to_point_pairs!();
}

impl<'p> PointPairCollection for Polygon<'p> {
    implement_to_point_pairs!();
}

impl<'r> PointPairCollection for Multipolygon<'r> {
    fn to_point_pairs(&self, zoom: u8) -> PointPairs {
        (0..self.polygon_count())
            .flat_map(|idx| self.get_polygon(idx).to_point_pairs(zoom))
            .collect()
    }
}
