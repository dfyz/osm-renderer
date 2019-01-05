use crate::mapcss::color::Color;

use crate::draw::TILE_SIZE;

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

#[derive(Default)]
pub struct TilePixels {
    pixels: Vec<RgbaColor>,
}

pub fn dimension() -> usize {
    TILE_SIZE
}

pub type RgbTriples = Vec<(u8, u8, u8)>;

impl TilePixels {
    pub fn new() -> TilePixels {
        TilePixels {
            pixels: vec![
                RgbaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };
                TILE_SIZE * TILE_SIZE
            ],
        }
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: &RgbaColor) {
        let idx = to_idx(x, y);
        let new_pixel = {
            let old_pixel = &self.pixels[idx];
            let blend = |new_value, old_value| new_value + (1.0 - color.a) * old_value;
            RgbaColor {
                r: blend(color.r, old_pixel.r),
                g: blend(color.g, old_pixel.g),
                b: blend(color.b, old_pixel.b),
                a: blend(color.a, old_pixel.a),
            }
        };
        self.pixels[idx] = new_pixel;
    }

    pub fn to_rgb_triples(&self) -> RgbTriples {
        let mut result = Vec::new();

        for p in &self.pixels {
            let postdivide = |val| {
                let mul = if p.a == 0.0 { 0.0 } else { val / p.a };
                (f64::from(u8::max_value()) * mul) as u8
            };
            result.push((postdivide(p.r), postdivide(p.g), postdivide(p.b)));
        }

        result
    }
}

fn to_idx(x: usize, y: usize) -> usize {
    (y * TILE_SIZE) + x
}

fn component_to_opacity(comp: u8) -> f64 {
    f64::from(comp) / f64::from(u8::max_value())
}
