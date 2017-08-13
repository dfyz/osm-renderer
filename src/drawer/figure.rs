use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

#[derive(Default)]
pub struct Figure {
    pub pixels: BTreeMap<usize, BTreeMap<usize, f64>>,
}

impl Figure {
    pub fn add(&mut self, x: usize, y: usize, opacity: f64) {
        match self.pixels.entry(x).or_insert(Default::default()).entry(y) {
            Entry::Occupied(o) => {
                *o.into_mut() = o.get().max(opacity);
            },
            Entry::Vacant(v) => {
                v.insert(opacity);
            },
        }
    }
}
