use coords::Coords;

use std::f64::consts::PI;

pub const MAX_ZOOM: u8 = 18;
pub const TILE_SIZE: u32 = 256;

#[derive(Eq, PartialEq, Debug)]
pub struct Tile {
    pub zoom: u8,
    pub x: u32,
    pub y: u32,
}

#[derive(Eq, PartialEq, Debug)]
pub struct TileRange {
    pub min_x: u32,
    pub max_x: u32,
    pub min_y: u32,
    pub max_y: u32,
}

/// Returns the smallest possible tile that contains a given geopoint.
/// # Examples
/// ```
/// use renderer::tile::{coords_to_max_zoom_tile,Tile};
/// assert_eq!(coords_to_max_zoom_tile(&(55.747764f64, 37.437745f64)), Tile { zoom: 18, x: 158333, y: 81957 });
/// assert_eq!(coords_to_max_zoom_tile(&(40.1222f64, 20.6852f64)), Tile { zoom: 18, x: 146134, y: 99125 });
/// assert_eq!(coords_to_max_zoom_tile(&(-35.306536f64, 149.126545f64)), Tile { zoom: 18, x: 239662, y: 158582 });
/// ```
pub fn coords_to_max_zoom_tile<C: Coords>(coords: &C) -> Tile {
    let (x, y) = coords_to_xy(coords, MAX_ZOOM);
    let tile_index = |t| t / TILE_SIZE;
    Tile {
        zoom: MAX_ZOOM,
        x: tile_index(x),
        y: tile_index(y),
    }
}

/// Return the range of all smallest tiles that are covered by a given tile.
/// # Examples
/// ```
/// use renderer::tile::{tile_to_max_zoom_tile_range,Tile,TileRange};
/// assert_eq!(tile_to_max_zoom_tile_range(&Tile { zoom: 0, x: 0, y: 0 }), TileRange {
///     min_x: 0,
///     max_x: 262143,
///     min_y: 0,
///     max_y: 262143,
/// });
/// assert_eq!(tile_to_max_zoom_tile_range(&Tile { zoom: 15, x: 19805, y: 10244 }), TileRange {
///     min_x: 158440,
///     max_x: 158447,
///     min_y: 81952,
///     max_y: 81959,
/// });
/// assert_eq!(tile_to_max_zoom_tile_range(&Tile { zoom: 18, x: 239662, y: 158582 }), TileRange {
///     min_x: 239662,
///     max_x: 239662,
///     min_y: 158582,
///     max_y: 158582,
/// });
/// ```
pub fn tile_to_max_zoom_tile_range(tile: &Tile) -> TileRange {
    let blow_up = |x| x * (1 << (MAX_ZOOM - tile.zoom));
    let (min_x, min_y) = (blow_up(tile.x), blow_up(tile.y));
    let delta = blow_up(1) - 1;
    TileRange {
        min_x: min_x,
        max_x: min_x + delta,
        min_y: min_y,
        max_y: min_y + delta,
    }
}

/// Projects a given geopoint to Web Mercator coordinates for a given zoom level.
/// # Examples
/// ```
/// use renderer::tile::coords_to_xy;
/// assert_eq!(coords_to_xy(&(55.747764f64, 37.437745f64), 5), (4948, 2561));
/// assert_eq!(coords_to_xy(&(55.747764f64, 37.437745f64), 18), (40533333, 20981065));
/// assert_eq!(coords_to_xy(&(40.1222f64, 20.6852f64), 0), (143, 97));
/// assert_eq!(coords_to_xy(&(-35.306536f64, 149.126545f64), 10), (239663, 158582));
/// ```
pub fn coords_to_xy<C: Coords>(coords: &C, zoom: u8) -> (u32, u32) {
    let (lat_rad, lon_rad) = (coords.lat().to_radians(), coords.lon().to_radians());

    let x = lon_rad + PI;
    let y = PI - ((PI / 4f64) + (lat_rad / 2f64)).tan().ln();

    let rescale = |x: f64| {
        let factor = x / (2f64 * PI);
        let dimension_in_pixels = (TILE_SIZE * (1 << zoom)) as f64;
        (factor * dimension_in_pixels).round() as u32
    };

    (rescale(x), rescale(y))
}