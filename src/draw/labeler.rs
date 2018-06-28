use draw::figure::Figure;
use draw::font::text_placer::TextPlacer;
use draw::icon::Icon;
use draw::labelable::Labelable;
use mapcss::styler::Style;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct Labeler {
    icon_cache: RwLock<HashMap<String, Option<Icon>>>,
    base_path: PathBuf,
    text_placer: TextPlacer,
}

impl Labeler {
    pub fn new(base_path: &Path) -> Labeler {
        Labeler {
            icon_cache: Default::default(),
            base_path: base_path.to_owned(),
            text_placer: TextPlacer::new(),
        }
    }

    pub fn label_entity(&self, entity: &impl Labelable, style: &Style, zoom: u8, figure: &mut Figure) {
        let mut label_figure = figure.clean_copy();
        let y_offset = self.label_with_icon(entity, style, zoom, &mut label_figure);
        self.label_with_text(entity, style, zoom, y_offset, &mut label_figure);
        figure.update_from(&label_figure);
    }

    fn label_with_icon(&self, entity: &impl Labelable, style: &Style, zoom: u8, figure: &mut Figure) -> usize {
        let icon_name = match style.icon_image {
            Some(ref icon_name) => {
                self.load_icon(icon_name);
                icon_name
            }
            _ => return 0,
        };

        let (center_x, center_y) = match entity.get_center(zoom) {
            Some(center) => center,
            _ => return 0,
        };

        let read_icon_cache = self.icon_cache.read().unwrap();
        if let Some(Some(icon)) = read_icon_cache.get(icon_name) {
            self.draw_icon(icon, center_x, center_y, figure);
            icon.height / 2
        } else {
            0
        }
    }

    fn label_with_text(&self, entity: &impl Labelable, style: &Style, zoom: u8, y_offset: usize, figure: &mut Figure) {
        if let Some(ref text) = style.text {
            if let Some(ref text_pos) = style.text_position {
                if let Some(font_size) = style.font_size {
                    self.text_placer
                        .place(entity, text, text_pos, font_size, zoom, y_offset, figure);
                }
            }
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

    fn load_icon(&self, icon_name: &str) {
        if self.icon_cache.read().unwrap().get(icon_name).is_some() {
            return;
        }

        let full_icon_path = self.base_path.join(icon_name);
        let mut write_icon_cache = self.icon_cache.write().unwrap();
        write_icon_cache
            .entry(icon_name.to_string())
            .or_insert(match Icon::load(&full_icon_path) {
                Ok(icon) => Some(icon),
                Err(error) => {
                    let full_icon_path_str = full_icon_path.to_str().unwrap_or("N/A");
                    eprintln!("Failed to load icon from {}: {}", full_icon_path_str, error);
                    None
                }
            });
    }
}
