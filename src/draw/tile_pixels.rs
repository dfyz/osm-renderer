use crate::draw::TILE_SIZE;
use crate::mapcss::color::Color;
use crate::tile::Tile;

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
    bb: BoundingBox,
    labels_bb: BoundingBox,
    next_pixels: Vec<Option<NextPixel>>,
    generation: usize,
    label_generation_statuses: Vec<bool>,
    scaled_tile_size: usize,
    scaled_extended_tile_size: usize,
}

#[derive(Clone)]
struct NextPixel {
    color: RgbaColor,
    generation: usize,
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
    pub fn new(tile: &Tile, scale: usize) -> TilePixels {
        let scaled_tile_size = TILE_SIZE * scale;

        let to_tile_start = |c| (c as usize) * scaled_tile_size;
        let to_tile_end = |tile_start_c| tile_start_c + scaled_tile_size - 1;
        let (tile_start_x, tile_start_y) = (to_tile_start(tile.x), to_tile_start(tile.y));
        let bounding_box = BoundingBox {
            min_x: tile_start_x,
            max_x: to_tile_end(tile_start_x),
            min_y: tile_start_y,
            max_y: to_tile_end(tile_start_y),
        };
        let bounding_box_for_labels = BoundingBox {
            min_x: bounding_box.min_x - scaled_tile_size,
            max_x: bounding_box.max_x + scaled_tile_size,
            min_y: bounding_box.min_y - scaled_tile_size,
            max_y: bounding_box.max_y + scaled_tile_size,
        };

        let scaled_extended_tile_size = EXTENDED_TILE_SIZE * scale;
        let pixel_count = scaled_extended_tile_size * scaled_extended_tile_size;

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
            bb: bounding_box,
            labels_bb: bounding_box_for_labels,
            next_pixels: vec![None; pixel_count],
            generation: 0,
            label_generation_statuses: Vec::new(),
            scaled_tile_size,
            scaled_extended_tile_size,
        }
    }

    pub fn fill(&mut self, color: &RgbaColor) {
        for pixel in self.pixels.iter_mut() {
            *pixel = color.clone();
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

    pub fn to_rgb_triples(&self) -> RgbTriples {
        let mut triples = Vec::new();

        let non_label_pixel_range = || self.scaled_tile_size..2 * self.scaled_tile_size;

        for y in non_label_pixel_range() {
            for x in non_label_pixel_range() {
                let p = &self.pixels[self.local_coords_to_idx(x, y)];
                let postdivide = |val| {
                    let mul = if p.a == 0.0 { 0.0 } else { val / p.a };
                    (f64::from(u8::max_value()) * mul) as u8
                };
                triples.push((postdivide(p.r), postdivide(p.g), postdivide(p.b)));
            }
        }

        triples
    }

    pub fn dimension(&self) -> usize {
        self.scaled_tile_size
    }

    pub fn bb(&self) -> &BoundingBox {
        &self.bb
    }

    fn global_coords_to_idx(&self, x: usize, y: usize, for_labels: bool) -> Option<usize> {
        let bb = if for_labels { &self.labels_bb } else { &self.bb };
        if x < bb.min_x || x > bb.max_x || y < bb.min_y || y > bb.max_y {
            return None;
        }
        Some(self.local_coords_to_idx(x - self.labels_bb.min_x, y - self.labels_bb.min_y))
    }

    fn local_coords_to_idx(&self, x: usize, y: usize) -> usize {
        y * self.scaled_extended_tile_size + x
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
}

fn component_to_opacity(comp: u8) -> f64 {
    f64::from(comp) / f64::from(u8::max_value())
}

const EXTENDED_TILE_SIZE: usize = 3 * TILE_SIZE;
