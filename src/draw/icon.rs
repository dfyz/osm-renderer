use crate::draw::tile_pixels::RgbaColor;
use anyhow::{bail, Context, Result};
use png::{ColorType, Decoder, Transformations};
use std::fs::File;
use std::path::Path;

pub struct Icon {
    pixels: Vec<RgbaColor>,
    pub width: usize,
    pub height: usize,
}

impl Icon {
    pub fn load<P>(icon_path: P) -> Result<Icon>
    where
        P: AsRef<Path>,
    {
        let icon_file = File::open(&icon_path).context("Failed to open icon file")?;
        let mut decoder = Decoder::new(icon_file);
        decoder.set_transformations(Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().context("Icon is not a valid PNG file")?;

        let mut pixels = Vec::<RgbaColor>::default();
        let mut raw_pixels = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut raw_pixels)
            .context("Failed to read PNG pixels")?;

        let mut idx = 0;
        while idx < info.buffer_size() {
            let (r, g, b, a, idx_delta) = match info.color_type {
                ColorType::Rgb => (raw_pixels[idx], raw_pixels[idx + 1], raw_pixels[idx + 2], u8::MAX, 3),
                ColorType::Rgba => (
                    raw_pixels[idx],
                    raw_pixels[idx + 1],
                    raw_pixels[idx + 2],
                    raw_pixels[idx + 3],
                    4,
                ),
                ColorType::GrayscaleAlpha => (
                    raw_pixels[idx],
                    raw_pixels[idx],
                    raw_pixels[idx],
                    raw_pixels[idx + 1],
                    2,
                ),
                unknown_color => bail!("Unknown color type: {:?}", unknown_color),
            };
            pixels.push(RgbaColor::from_components(r, g, b, a));
            idx += idx_delta;
        }

        Ok(Icon {
            pixels,
            width: info.width as usize,
            height: info.height as usize,
        })
    }

    pub fn get(&self, x: usize, y: usize) -> RgbaColor {
        self.pixels[y * self.width + x].clone()
    }
}
