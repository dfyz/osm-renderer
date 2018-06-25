use draw::point::Point;
use geodata::reader::{Node, Relation, Way};

#[derive(Eq, PartialEq, Hash)]
pub struct NodePair<'n> {
    n1: Node<'n>,
    n2: Node<'n>,
}

impl<'n> NodePair<'n> {
    pub fn to_points(&self, zoom: u8) -> (Point, Point) {
        (Point::from_node(&self.n1, zoom), Point::from_node(&self.n2, zoom))
    }

    pub fn reverse(&self) -> NodePair<'n> {
        NodePair {
            n1: self.n2.clone(),
            n2: self.n1.clone(),
        }
    }
}

pub trait NodePairCollection<'a> {
    fn to_node_pairs(&self) -> Vec<NodePair<'a>>;
}

impl<'w> NodePairCollection<'w> for Way<'w> {
    fn to_node_pairs(&self) -> Vec<NodePair<'w>> {
        (1..self.node_count())
            .map(|idx| NodePair {
                n1: self.get_node(idx - 1),
                n2: self.get_node(idx),
            })
            .collect()
    }
}

impl<'r> NodePairCollection<'r> for Relation<'r> {
    fn to_node_pairs(&self) -> Vec<NodePair<'r>> {
        (0..self.way_count())
            .flat_map(|idx| self.get_way(idx).to_node_pairs())
            .collect()
    }
}
