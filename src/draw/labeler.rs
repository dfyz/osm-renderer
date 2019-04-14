use crate::draw::font::text_placer::TextPlacer;
use crate::draw::icon::Icon;
use crate::draw::icon_cache::IconCache;
use crate::draw::labelable::Labelable;
use crate::draw::tile_pixels::TilePixels;
use crate::geodata::reader::OsmEntity;
use crate::mapcss::styler::{Style, TextPosition};

#[derive(Default)]
pub struct Labeler {
    text_placer: TextPlacer,
}

impl Labeler {
    pub fn label_entity<'e, E>(
        &self,
        entity: &E,
        style: &Style,
        zoom: u8,
        scale: f64,
        icon_cache: &IconCache,
        default_text_position: TextPosition,
        pixels: &mut TilePixels,
    ) where
        E: Labelable + OsmEntity<'e>,
    {
        let succeeded = {
            if let Some(y_offset) = self.label_with_icon(entity, style, zoom, scale, icon_cache, pixels) {
                self.label_with_text(entity, style, zoom, scale, y_offset, default_text_position, pixels)
            } else {
                false
            }
        };

        pixels.bump_label_generation(succeeded);
    }

    fn label_with_icon(
        &self,
        entity: &impl Labelable,
        style: &Style,
        zoom: u8,
        scale: f64,
        icon_cache: &IconCache,
        pixels: &mut TilePixels,
    ) -> Option<usize> {
        let icon_name = match style.icon_image {
            Some(ref icon_name) => icon_name,
            _ => return Some(0),
        };

        let read_icon_cache = icon_cache.open_read_session(icon_name);

        if let Some(Some(icon)) = read_icon_cache.get(icon_name) {
            let (center_x, center_y) = match entity.get_label_position(zoom, scale) {
                Some(center) => center,
                _ => return Some(0),
            };
            if self.draw_icon(icon, center_x, center_y, pixels) {
                Some(icon.height / 2)
            } else {
                None
            }
        } else {
            Some(0)
        }
    }

    fn label_with_text<'e, E>(
        &self,
        entity: &E,
        style: &Style,
        zoom: u8,
        scale: f64,
        y_offset: usize,
        default_text_position: TextPosition,
        pixels: &mut TilePixels,
    ) -> bool
    where
        E: Labelable + OsmEntity<'e>,
    {
        if let Some(ref text_style) = style.text_style {
            self.text_placer
                .place(entity, text_style, zoom, scale, y_offset, default_text_position, pixels)
        } else {
            true
        }
    }

    fn draw_icon(&self, icon: &Icon, center_x: f64, center_y: f64, pixels: &mut TilePixels) -> bool {
        let get_start_coord = |coord, dimension| (coord - (dimension as f64 / 2.0)) as usize;

        let start_x = get_start_coord(center_x, icon.width);
        let start_y = get_start_coord(center_y, icon.height);

        for x in 0..icon.width {
            for y in 0..icon.height {
                if !pixels.set_label_pixel(start_x + x, start_y + y, &icon.get(x, y)) {
                    return false;
                }
            }
        }

        true
    }
}
