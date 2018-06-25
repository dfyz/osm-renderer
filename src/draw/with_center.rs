use draw::point::Point;
use geodata::reader::{Node, Relation, Way};

type Center = Option<(f64, f64)>;

pub trait WithCenter {
    fn get_center(&self, zoom: u8) -> Center;
}

impl<'n> WithCenter for Node<'n> {
    fn get_center(&self, zoom: u8) -> Center {
        let center = Point::from_node(self, zoom);
        Some((f64::from(center.x), f64::from(center.y)))
    }
}

impl<'w> WithCenter for Way<'w> {
    fn get_center(&self, zoom: u8) -> Center {
        let way_nodes = (0..self.node_count()).map(|idx| self.get_node(idx));
        get_centroid(way_nodes, zoom)
    }
}

impl<'r> WithCenter for Relation<'r> {
    fn get_center(&self, zoom: u8) -> Center {
        let relation_nodes = (0..self.way_count()).flat_map(|way_idx| {
            let way = self.get_way(way_idx);
            (0..way.node_count()).map(move |node_idx| way.get_node(node_idx))
        });
        get_centroid(relation_nodes, zoom)
    }
}

fn get_centroid<'n>(nodes: impl Iterator<Item = Node<'n>>, zoom: u8) -> Center {
    let mut x = 0.0;
    let mut y = 0.0;
    let mut node_count = 0;
    for node in nodes {
        let point = Point::from_node(&node, zoom);
        x += f64::from(point.x);
        y += f64::from(point.y);
        node_count += 1;
    }
    if node_count == 0 {
        None
    } else {
        let norm = node_count as f64;
        x /= norm;
        y /= norm;
        Some((x, y))
    }
}
