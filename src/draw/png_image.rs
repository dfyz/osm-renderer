use errors::*;

use mapcss::color::Color;
use png::{ColorType, Encoder, HasParameters};
use tile as t;

use draw::TILE_SIZE;

#[derive(Clone)]
pub struct RgbaColor {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl RgbaColor {
    pub fn from_color(color: &Color, opacity: f64) -> RgbaColor {
        let premultiply = |c| opacity * (f64::from(c) / f64::from(u8::max_value()));

        RgbaColor {
            r: premultiply(color.r),
            g: premultiply(color.g),
            b: premultiply(color.b),
            a: opacity,
        }
    }
}

#[derive(Default)]
pub struct PngImage {
    pixels: Vec<RgbaColor>,
}

impl PngImage {
    pub fn new() -> PngImage {
        PngImage {
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

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        {
            let mut png_encoder = Encoder::new(&mut buf, t::TILE_SIZE, t::TILE_SIZE);
            png_encoder.set(ColorType::RGB);
            let mut png_writer = png_encoder
                .write_header()
                .chain_err(|| "Failed to write PNG header")?;

            let mut image_bytes = Vec::new();
            for p in &self.pixels {
                let postdivide = |val| {
                    let mul = if p.a == 0.0 {
                        0.0
                    } else {
                        val / p.a
                    };
                    (f64::from(u8::max_value()) * mul) as u8
                };
                image_bytes.extend([postdivide(p.r), postdivide(p.g), postdivide(p.b)].into_iter());
            }
            png_writer
                .write_image_data(image_bytes.as_slice())
                .chain_err(|| "Failed to write PNG data")?;
        }
        Ok(buf)
    }
}

fn to_idx(x: usize, y: usize) -> usize {
    (y * TILE_SIZE) + x
}
