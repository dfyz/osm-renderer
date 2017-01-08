use coords::Coords;

pub const MAX_ZOOM: u8 = 18;

pub struct Tile {
    zoom: u8,
    x: u32,
    y: u32,
}

pub fn coords_to_max_zoom_tile<C: Coords>(coords: &C) -> Tile {
}

pub fn tile_to_max_zoom_tiles(tile: &Tile) -> Iterator<Item=Tile> {
}

pub fn coords_to_xy<C: Coords>(coords: &C, zoom: u8) -> (u32, u32) {
}
