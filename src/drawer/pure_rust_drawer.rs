use errors::*;

use geodata::reader::OsmEntities;
use mapcss::color::Color;
use mapcss::styler::Styler;
use png::{ColorType, Encoder, HasParameters};
use std::io::Cursor;
use tile as t;

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &t::Tile, styler: &Styler) -> Result<Vec<u8>> {
    let mut image = PngImage::new();
    if let Some(c) = styler.canvas_fill_color {
        for x in 0..TILE_SIZE {
            for y in 0..TILE_SIZE {
                image.set_pixel(x, y, &c);
            }
        }
    }
    image.to_bytes()
}

const TILE_SIZE: usize = t::TILE_SIZE as usize;
const TOTAL_PIXELS: usize = TILE_SIZE * TILE_SIZE;

struct PngImage {
    pixels: Vec<Color>,
}

impl PngImage {
    fn new() -> PngImage {
        PngImage {
            pixels: vec![
                Color { r: 0, g: 0, b: 0 };
                TOTAL_PIXELS
            ],
        }
    }

    fn set_pixel(&mut self, x: usize, y: usize, color: &Color) {
        self.pixels[(y * TILE_SIZE) + x] = color.clone();
    }

    fn to_bytes(self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        {
            let mut png_encoder = Encoder::new(&mut buf, t::TILE_SIZE, t::TILE_SIZE);
            png_encoder.set(ColorType::RGBA);
            let mut png_writer = png_encoder.write_header().chain_err(|| "Failed to write PNG header")?;

            let mut image_bytes = Vec::new();
            for p in self.pixels {
                image_bytes.extend([p.r, p.g, p.b, u8::max_value()].into_iter());
            }
            png_writer.write_image_data(image_bytes.as_slice()).chain_err(|| "Failed to write PNG data")?;
        }
        Ok(buf)
    }
}
