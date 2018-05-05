use draw::tile_pixels::RgbaColor;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

pub struct BoundingBox {
    pub min_x: usize,
    pub max_x: usize,
    pub min_y: usize,
    pub max_y: usize,
}

pub struct Figure {
    pub pixels: BTreeMap<usize, BTreeMap<usize, RgbaColor>>,
    pub bounding_box: BoundingBox,
}

impl Figure {
    pub fn new(bounding_box: BoundingBox) -> Figure {
        Figure {
            pixels: Default::default(),
            bounding_box,
        }
    }

    pub fn add(&mut self, x: usize, y: usize, color: RgbaColor) {
        let bb = &self.bounding_box;
        if x < bb.min_x || x > bb.max_x || y < bb.min_y || y > bb.max_y {
            return;
        }
        match self.pixels
            .entry(y)
            .or_insert_with(Default::default)
            .entry(x)
        {
            Entry::Occupied(o) => {
                if color.a > o.get().a {
                    *o.into_mut() = color;
                }
            }
            Entry::Vacant(v) => {
                v.insert(color);
            }
        }
    }
}
