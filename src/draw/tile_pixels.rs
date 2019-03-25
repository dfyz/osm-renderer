use crate::draw::TILE_SIZE;
use crate::mapcss::color::Color;
use crate::tile::Tile;
use std::ops::Range;

#[derive(Clone)]
pub struct RgbaColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl RgbaColor {
    pub fn from_color(color: &Color, opacity: f64) -> RgbaColor {
        let premultiply = |c| opacity * component_to_opacity(c);

        RgbaColor {
            r: premultiply(color.r),
            g: premultiply(color.g),
            b: premultiply(color.b),
            a: opacity,
        }
    }

    pub fn from_components(r: u8, g: u8, b: u8, a: u8) -> RgbaColor {
        RgbaColor::from_color(&Color { r, g, b }, component_to_opacity(a))
    }
}

pub struct TilePixels {
    pixels: Vec<RgbaColor>,
    next_pixels: Vec<Option<NextPixel>>,
    bb: BoundingBox,
    labels_bb: BoundingBox,
    generation: usize,
    label_generation_statuses: Vec<bool>,
}

#[derive(Clone)]
struct NextPixel {
    color: RgbaColor,
    generation: usize,
}

pub fn dimension() -> usize {
    TILE_SIZE
}

pub type RgbTriples = Vec<(u8, u8, u8)>;

#[derive(Clone)]
pub struct BoundingBox {
    pub min_x: usize,
    pub max_x: usize,
    pub min_y: usize,
    pub max_y: usize,
}

impl TilePixels {
    pub fn new(tile: &Tile) -> TilePixels {
        let to_tile_start = |c| (c as usize) * TILE_SIZE;
        let to_tile_end = |tile_start_c| tile_start_c + TILE_SIZE - 1;
        let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));
        let bounding_box = BoundingBox {
            min_x: tile_start_x,
            max_x: to_tile_end(tile_start_x),
            min_y: tile_start_y,
            max_y: to_tile_end(tile_start_y),
        };
        let bounding_box_for_labels = BoundingBox {
            min_x: bounding_box.min_x - TILE_SIZE,
            max_x: bounding_box.max_x + TILE_SIZE,
            min_y: bounding_box.min_y - TILE_SIZE,
            max_y: bounding_box.max_y + TILE_SIZE,
        };

        let pixel_count = EXTENDED_TILE_SIZE * EXTENDED_TILE_SIZE;
        TilePixels {
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };
                pixel_count
            ],
            next_pixels: vec![None; pixel_count],
            bb: bounding_box,
            labels_bb: bounding_box_for_labels,
            generation: 0,
            label_generation_statuses: Vec::new(),
        }
    }

    pub fn fill(&mut self, color: &RgbaColor) {
        for y in NON_LABEL_PIXEL_RANGE {
            for x in NON_LABEL_PIXEL_RANGE {
                let idx = self.local_coords_to_idx(x, y);
                self.pixels[idx] = color.clone();
            }
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: &RgbaColor) {
        let idx = match self.global_coords_to_idx(x, y, false) {
            Some(idx) => idx,
            _ => return,
        };
        let mut from_same_generation = false;
        if let Some(next_pixel) = &mut self.next_pixels[idx] {
            if next_pixel.generation == self.generation {
                if color.a > next_pixel.color.a {
                    next_pixel.color = color.clone();
                }
                from_same_generation = true;
            }
        }
        if !from_same_generation {
            self.blend_pixel(idx, false);
            self.next_pixels[idx] = Some(NextPixel {
                color: color.clone(),
                generation: self.generation,
            });
        }
    }

    pub fn set_label_pixel(&mut self, x: usize, y: usize, color: &RgbaColor) -> bool {
        let idx = match self.global_coords_to_idx(x, y, true) {
            Some(idx) => idx,
            _ => return true,
        };

        let label_generation = self.label_generation_statuses.len();
        if let Some(next_pixel) = &mut self.next_pixels[idx] {
            if next_pixel.generation < label_generation && self.label_generation_statuses[next_pixel.generation] {
                return false;
            }
        }
        self.next_pixels[idx] = Some(NextPixel {
            color: color.clone(),
            generation: label_generation,
        });
        true
    }

    pub fn to_rgb_triples(&self) -> RgbTriples {
        let mut result = Vec::new();

        for y in NON_LABEL_PIXEL_RANGE {
            for x in NON_LABEL_PIXEL_RANGE {
                let p = &self.pixels[self.local_coords_to_idx(x, y)];
                let postdivide = |val| {
                    let mul = if p.a == 0.0 { 0.0 } else { val / p.a };
                    (f64::from(u8::max_value()) * mul) as u8
                };
                result.push((postdivide(p.r), postdivide(p.g), postdivide(p.b)));
            }
        }

        result
    }

    pub fn bb(&self) -> &BoundingBox {
        &self.labels_bb
    }

    pub fn bump_generation(&mut self) {
        self.generation += 1;
    }

    pub fn blend_unfinished_pixels(&mut self, for_labels: bool) {
        for idx in 0..self.next_pixels.len() {
            self.blend_pixel(idx, for_labels);
        }
    }

    pub fn bump_label_generation(&mut self, succeeded: bool) {
        self.label_generation_statuses.push(succeeded);
    }

    fn blend_pixel(&mut self, idx: usize, for_labels: bool) {
        let next_pixel_ref = &mut self.next_pixels[idx];
        if let Some(next_pixel) = next_pixel_ref {
            if !for_labels || self.label_generation_statuses[next_pixel.generation] {
                let old_pixel = &mut self.pixels[idx];
                let new_pixel = {
                    let blend = |new_value, old_value| new_value + (1.0 - next_pixel.color.a) * old_value;
                    RgbaColor {
                        r: blend(next_pixel.color.r, old_pixel.r),
                        g: blend(next_pixel.color.g, old_pixel.g),
                        b: blend(next_pixel.color.b, old_pixel.b),
                        a: blend(next_pixel.color.a, old_pixel.a),
                    }
                };
                *old_pixel = new_pixel;
            }
        }
        next_pixel_ref.take();
    }

    fn global_coords_to_idx(&self, x: usize, y: usize, for_labels: bool) -> Option<usize> {
        let bb = if for_labels { &self.labels_bb } else { &self.bb };
        if x < bb.min_x || x > bb.max_x || y < bb.min_y || y > bb.max_y {
            return None;
        }
        Some(self.local_coords_to_idx(x - self.labels_bb.min_x, y - self.labels_bb.min_y))
    }

    fn local_coords_to_idx(&self, x: usize, y: usize) -> usize {
        y * EXTENDED_TILE_SIZE + x
    }
}

fn component_to_opacity(comp: u8) -> f64 {
    f64::from(comp) / f64::from(u8::max_value())
}

const NON_LABEL_PIXEL_RANGE: Range<usize> = TILE_SIZE..2 * TILE_SIZE;
const EXTENDED_TILE_SIZE: usize = 3 * TILE_SIZE;
