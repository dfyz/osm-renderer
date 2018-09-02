use draw::point::Point;
use geodata::reader::{Multipolygon, Node, Polygon, Way};

#[derive(Eq, PartialEq, Hash)]
pub struct NodePair<'n> {
    n1: Node<'n>,
    n2: Node<'n>,
}

impl<'n> NodePair<'n> {
    pub fn to_points(&self, zoom: u8) -> (Point, Point) {
        (Point::from_node(&self.n1, zoom), Point::from_node(&self.n2, zoom))
    }
}

pub trait NodePairCollection<'a> {
    fn to_node_pairs(&self) -> Vec<NodePair<'a>>;
}

macro_rules! implement_to_node_pairs {
    ($lft:lifetime) => {
        fn to_node_pairs(&self) -> Vec<NodePair<$lft>> {
            (1..self.node_count())
                .map(|idx| NodePair {
                    n1: self.get_node(idx - 1),
                    n2: self.get_node(idx),
                })
                .collect()
        }
    }
}

impl<'w> NodePairCollection<'w> for Way<'w> {
    implement_to_node_pairs!('w);
}

impl<'p> NodePairCollection<'p> for Polygon<'p> {
    implement_to_node_pairs!('p);
}

impl<'r> NodePairCollection<'r> for Multipolygon<'r> {
    fn to_node_pairs(&self) -> Vec<NodePair<'r>> {
        (0..self.polygon_count())
            .flat_map(|idx| self.get_polygon(idx).to_node_pairs())
            .collect()
    }
}
