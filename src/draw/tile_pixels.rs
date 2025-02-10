use crate::draw::TILE_SIZE;
use crate::mapcss::color::Color;

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
    bb: BoundingBox,
    labels_bb: BoundingBox,
    scaled_tile_size: usize,
    scaled_extended_tile_size: usize,
    pixels: Vec<RgbaColor>,
    next_pixels: Vec<Option<NextPixel>>,
    generation: usize,
    label_generation_statuses: Vec<bool>,
}

#[derive(Clone)]
struct NextPixel {
    color: RgbaColor,
    generation: usize,
}

pub type RgbTriples = Vec<(u8, u8, u8)>;

#[derive(Clone)]
pub struct BoundingBox {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

impl TilePixels {
    pub fn new(scale: usize) -> TilePixels {
        let scaled_tile_size = TILE_SIZE * scale;
        let scaled_tile_size_i32 = scaled_tile_size as i32;

        let bounding_box = BoundingBox {
            min_x: 0,
            max_x: scaled_tile_size_i32 - 1,
            min_y: 0,
            max_y: scaled_tile_size_i32 - 1,
        };
        let bounding_box_for_labels = BoundingBox {
            min_x: bounding_box.min_x - scaled_tile_size_i32,
            max_x: bounding_box.max_x + scaled_tile_size_i32,
            min_y: bounding_box.min_y - scaled_tile_size_i32,
            max_y: bounding_box.max_y + scaled_tile_size_i32,
        };

        let scaled_extended_tile_size = EXTENDED_TILE_SIZE * scale;
        let pixel_count = scaled_extended_tile_size * scaled_extended_tile_size;

        TilePixels {
            bb: bounding_box,
            labels_bb: bounding_box_for_labels,
            scaled_tile_size,
            scaled_extended_tile_size,
            pixels: vec![DEFAULT_PIXEL_COLOR; pixel_count],
            next_pixels: vec![None; pixel_count],
            generation: 0,
            label_generation_statuses: Vec::new(),
        }
    }

    pub fn reset(&mut self, canvas_color: &Option<Color>) {
        let initial_pixel_color = canvas_color
            .as_ref()
            .map(|c| RgbaColor::from_color(c, 1.0))
            .unwrap_or(DEFAULT_PIXEL_COLOR);

        for pixel in self.pixels.iter_mut() {
            *pixel = initial_pixel_color.clone();
        }

        for next_pixel in self.next_pixels.iter_mut() {
            next_pixel.take();
        }

        self.generation = 0;
        self.label_generation_statuses.clear();
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: &RgbaColor) {
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

    pub fn set_label_pixel(&mut self, x: i32, y: i32, color: &RgbaColor) -> bool {
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
                    (f64::from(u8::MAX) * mul) as u8
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

    fn global_coords_to_idx(&self, x: i32, y: i32, for_labels: bool) -> Option<usize> {
        let bb = if for_labels { &self.labels_bb } else { &self.bb };
        if x < bb.min_x || x > bb.max_x || y < bb.min_y || y > bb.max_y {
            return None;
        }
        let local_x = (x - self.labels_bb.min_x) as usize;
        let local_y = (y - self.labels_bb.min_y) as usize;
        Some(self.local_coords_to_idx(local_x, local_y))
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
    f64::from(comp) / f64::from(u8::MAX)
}

const EXTENDED_TILE_SIZE: usize = 3 * TILE_SIZE;
const DEFAULT_PIXEL_COLOR: RgbaColor = RgbaColor {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
