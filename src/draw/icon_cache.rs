use draw::icon::Icon;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard};

pub struct IconCache {
    cache: RwLock<NameToIcon>,
    base_path: PathBuf,
}

pub type NameToIcon = HashMap<String, Option<Icon>>;

impl IconCache {
    pub fn new(base_path: &Path) -> IconCache {
        IconCache {
            cache: RwLock::<NameToIcon>::default(),
            base_path: base_path.to_owned(),
        }
    }

    pub fn load_if_needed(&self, icon_name: &str) -> RwLockReadGuard<NameToIcon> {
        {
            let read_cache = self.cache.read().unwrap();
            if read_cache.get(icon_name).is_some() {
                return read_cache;
            }
        }

        {
            let full_icon_path = self.base_path.join(icon_name);
            let mut write_icon_cache = self.cache.write().unwrap();
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

        self.cache.read().unwrap()
    }
}
