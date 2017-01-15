use coords::Coords;

use std::f64::consts::PI;

pub const MAX_ZOOM: u8 = 18;
pub const TILE_SIZE: u32 = 256;

#[derive(Eq, PartialEq, Debug)]
pub struct Tile {
    zoom: u8,
    x: u32,
    y: u32,
}

pub fn coords_to_max_zoom_tile<C: Coords>(coords: &C) -> Tile {
    let (x, y) = coords_to_xy(coords, MAX_ZOOM);
    let tile_index = |t| t / TILE_SIZE;
    Tile {
        zoom: MAX_ZOOM,
        x: tile_index(x),
        y: tile_index(y),
    }
}

pub fn tile_to_max_zoom_tiles(tile: &Tile) -> TileIterator {
    TileIterator {}
}

pub fn coords_to_xy<C: Coords>(coords: &C, zoom: u8) -> (u32, u32) {
    let (lat_rad, lon_rad) = (coords.lat().to_radians(), coords.lon().to_radians());

    let x = lon_rad + PI;
    let y = PI - ((PI / 4f64) + (lat_rad / 2f64)).tan().ln();

    let rescale = |x| {
        let factor = x / (2f64 * PI);
        let dimension_in_pixels = (TILE_SIZE * (1 << zoom)) as f64;
        (factor * dimension_in_pixels) as u32
    };

    (rescale(x), rescale(y))
}

pub struct TileIterator {
}

impl Iterator for TileIterator {
    type Item = Tile;

    fn next(&mut self) -> Option<Tile> {
        None
    }
}

#[cfg(test)]
mod tests {
    use tile::*;

    #[test]
    fn test_coords_to_max_zoom_tile() {
        assert_eq!(coords_to_max_zoom_tile(&(55.747764f64, 37.437745f64)), Tile { zoom: 18, x: 158333, y: 81957 });
        assert_eq!(coords_to_max_zoom_tile(&(40.1222f64, 20.6852f64)), Tile { zoom: 18, x: 146134, y: 99125 });
        assert_eq!(coords_to_max_zoom_tile(&(-35.306536f64, 149.126545f64)), Tile { zoom: 18, x: 239662, y: 158582 });
    }

    #[test]
    fn test_coords_to_xy() {
        assert_eq!(coords_to_xy(&(55.747764f64, 37.437745f64), 18), (0, 0));
    }
}
