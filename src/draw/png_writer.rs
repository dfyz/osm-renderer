use anyhow::{Context, Result};
use png::{ColorType, Encoder};

pub fn rgb_triples_to_png(triples: &[(u8, u8, u8)], width: usize, height: usize) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut png_encoder = Encoder::new(&mut buf, width as u32, height as u32);
        png_encoder.set_color(ColorType::RGB);
        let mut png_writer = png_encoder.write_header().context("Failed to write PNG header")?;

        let mut image_bytes = Vec::new();
        for &(r, g, b) in triples {
            image_bytes.extend([r, g, b].iter());
        }

        png_writer
            .write_image_data(image_bytes.as_slice())
            .context("Failed to write PNG data")?;
    }
    Ok(buf)
}
