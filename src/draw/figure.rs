use draw::png_image::RgbaColor;

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

#[derive(Default)]
pub struct Figure {
    pub pixels: BTreeMap<usize, BTreeMap<usize, RgbaColor>>,
}

impl Figure {
    pub fn add(&mut self, x: usize, y: usize, color: RgbaColor) {
        match self.pixels.entry(y).or_insert(Default::default()).entry(x) {
            Entry::Occupied(o) => {
                if color.a > o.get().a {
                    *o.into_mut() = color;
                }
            },
            Entry::Vacant(v) => {
                v.insert(color);
            },
        }
    }
}
