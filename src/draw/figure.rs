use crate::draw::tile_pixels::RgbaColor;
use crate::draw::TILE_SIZE;
use crate::tile::Tile;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

#[derive(Clone)]
pub struct BoundingBox {
    pub min_x: usize,
    pub max_x: usize,
    pub min_y: usize,
    pub max_y: usize,
}

pub struct Figure {
    pub pixels: Pixels,
    pub bounding_box: BoundingBox,
}

impl Figure {
    pub fn new(tile: &Tile) -> Figure {
        let to_tile_start = |c| (c as usize) * TILE_SIZE;
        let to_tile_end = |tile_start_c| tile_start_c + TILE_SIZE - 1;
        let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));
        let bounding_box = BoundingBox {
            min_x: tile_start_x - TILE_SIZE,
            max_x: to_tile_end(tile_start_x) + TILE_SIZE,
            min_y: tile_start_y - TILE_SIZE,
            max_y: to_tile_end(tile_start_y) + TILE_SIZE,
        };

        Figure {
            pixels: Pixels::default(),
            bounding_box,
        }
    }

    pub fn clean_copy(&self) -> Figure {
        Figure {
            pixels: Pixels::default(),
            bounding_box: self.bounding_box.clone(),
        }
    }

    pub fn add(&mut self, x: usize, y: usize, color: RgbaColor) {
        let bb = &self.bounding_box;
        if x < bb.min_x || x > bb.max_x || y < bb.min_y || y > bb.max_y {
            return;
        }
        match self.pixels.entry(y).or_insert_with(Default::default).entry(x) {
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

    pub fn update_from(&mut self, other: &Figure) {
        for (other_y, other_x_to_color) in &other.pixels {
            if other_x_to_color.is_empty() {
                continue;
            }
            if let Some(our_x_to_color) = self.pixels.get(other_y) {
                if our_x_to_color
                    .range(other_x_to_color.keys().min().unwrap()..=other_x_to_color.keys().max().unwrap())
                    .next()
                    .is_some()
                {
                    return;
                }
            }
        }
        for (other_y, other_x_to_color) in &other.pixels {
            for (other_x, other_color) in other_x_to_color.iter() {
                self.add(*other_x, *other_y, other_color.clone());
            }
        }
    }
}

type Pixels = BTreeMap<usize, BTreeMap<usize, RgbaColor>>;
