use draw::figure::Figure;
use draw::icon::Icon;
use draw::with_center::WithCenter;
use mapcss::styler::Style;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

pub struct Labeler {
    icon_cache: RwLock<HashMap<String, Option<Icon>>>,
    base_path: PathBuf,
}

impl Labeler {
    pub fn new(base_path: &Path) -> Labeler {
        Labeler {
            icon_cache: Default::default(),
            base_path: base_path.to_owned(),
        }
    }

    pub fn label_entity(
        &self,
        entity: &impl WithCenter,
        style: &Style,
        zoom: u8,
        figure: &mut Figure,
    ) {
        let icon_name = match style.icon_image {
            Some(ref icon_name) => {
                self.load_icon(icon_name);
                icon_name
            },
            _ => return,
        };

        let (center_x, center_y) = match entity.get_center(zoom) {
            Some(center) => center,
            _ => return,
        };

        let read_icon_cache = self.icon_cache.read().unwrap();
        if let Some(Some(icon)) = read_icon_cache.get(icon_name) {
            let mut label_figure = figure.clean_copy();
            self.draw_icon(icon, center_x, center_y, &mut label_figure);
            figure.update_from(&label_figure);
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
