use crate::draw::figure::Figure;
use crate::draw::font::text_placer::TextPlacer;
use crate::draw::icon::Icon;
use crate::draw::icon_cache::IconCache;
use crate::draw::labelable::Labelable;
use crate::mapcss::styler::Style;

#[derive(Default)]
pub struct Labeler {
    text_placer: TextPlacer,
}

impl Labeler {
    pub fn label_entity(
        &self,
        entity: &impl Labelable,
        style: &Style,
        zoom: u8,
        icon_cache: &IconCache,
        figure: &mut Figure,
    ) {
        let mut label_figure = figure.clean_copy();
        let y_offset = self.label_with_icon(entity, style, zoom, icon_cache, &mut label_figure);
        self.label_with_text(entity, style, zoom, y_offset, &mut label_figure);
        figure.update_from(&label_figure);
    }

    fn label_with_icon(
        &self,
        entity: &impl Labelable,
        style: &Style,
        zoom: u8,
        icon_cache: &IconCache,
        figure: &mut Figure,
    ) -> usize {
        let icon_name = match style.icon_image {
            Some(ref icon_name) => icon_name,
            _ => return 0,
        };

        let read_icon_cache = icon_cache.open_read_session(icon_name);

        if let Some(Some(icon)) = read_icon_cache.get(icon_name) {
            let (center_x, center_y) = match entity.get_center(zoom) {
                Some(center) => center,
                _ => return 0,
            };
            self.draw_icon(icon, center_x, center_y, figure);
            icon.height / 2
        } else {
            0
        }
    }

    fn label_with_text(&self, entity: &impl Labelable, style: &Style, zoom: u8, y_offset: usize, figure: &mut Figure) {
        if let Some(ref text_style) = style.text_style {
            self.text_placer.place(entity, text_style, zoom, y_offset, figure);
        }
    }

    fn draw_icon(&self, icon: &Icon, center_x: f64, center_y: f64, figure: &mut Figure) {
        let get_start_coord = |coord, dimension| (coord - (dimension as f64 / 2.0)) as usize;

        let start_x = get_start_coord(center_x, icon.width);
        let start_y = get_start_coord(center_y, icon.height);

        for x in 0..icon.width {
            for y in 0..icon.height {
                figure.add(start_x + x, start_y + y, icon.get(x, y));
            }
        }
    }
}
