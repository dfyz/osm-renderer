use errors::*;

use draw::tile_pixels::RgbaColor;
use png::{ColorType, Decoder};
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
        let icon_file = File::open(&icon_path).chain_err(|| "Failed to open icon file")?;
        let decoder = Decoder::new(icon_file);
        let (info, mut reader) = decoder.read_info().chain_err(|| "Icon is not a valid PNG file")?;

        let mut pixels = Vec::<RgbaColor>::default();
        while let Some(row) = reader.next_row().chain_err(|| "Failed to read a PNG pixel")? {
            let mut idx = 0;
            while idx < row.len() {
                let (r, g, b, a, idx_delta) = match info.color_type {
                    ColorType::RGB => (row[idx], row[idx + 1], row[idx + 2], u8::max_value(), 3),
                    ColorType::RGBA => (row[idx], row[idx + 1], row[idx + 2], row[idx + 3], 4),
                    ColorType::GrayscaleAlpha => (row[idx], row[idx], row[idx], row[idx + 1], 2),
                    unknown_color => bail!("Unknown color type: {:?}", unknown_color),
                };
                pixels.push(RgbaColor::from_components(r, g, b, a));
                idx += idx_delta;
            }
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
