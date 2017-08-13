use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

#[derive(Default)]
pub struct DrawnPixels {
    pub x_to_y_opacities: BTreeMap<usize, BTreeMap<usize, f64>>,
}

impl DrawnPixels {
    pub fn add(&mut self, x: usize, y: usize, opacity: f64) {
        match self.x_to_y_opacities.entry(x).or_insert(Default::default()).entry(y) {
            Entry::Occupied(o) => {
                *o.into_mut() = o.get().max(opacity);
            },
            Entry::Vacant(v) => {
                v.insert(opacity);
            },
        }
    }
}
