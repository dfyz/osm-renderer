use errors::*;

use coords::Coords;
use cs;
use geodata::reader::OsmEntities;
use libc;
use std::f64::consts::PI;
use std::slice;
use tile::{coords_to_xy, Tile, TILE_SIZE};

unsafe extern "C" fn write_func(closure: *mut libc::c_void, data: *mut u8, len: libc::c_uint) -> cs::enums::Status {
    let png_bytes: &mut Vec<u8> = &mut *(closure as *mut Vec<u8>);
    png_bytes.extend(slice::from_raw_parts(data, len as usize));
    cs::enums::Status::Success
}

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &Tile) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    unsafe {
        let s = cs::cairo_image_surface_create(cs::enums::Format::Rgb24, TILE_SIZE as i32, TILE_SIZE as i32);

        let cr = cs::cairo_create(s);

        let translate_coord = |c, tile_c| (c - TILE_SIZE * tile_c) as f64;
        for n in entities.nodes.iter() {
            let coords = (n.lat(), n.lon());
            let (x, y) = coords_to_xy(&coords, tile.zoom);
            let real_x = translate_coord(x, tile.x);
            let real_y = translate_coord(y, tile.y);

            cs::cairo_arc(cr, real_x, real_y, 4.0, 0.0, 2.0*PI);
            cs::cairo_set_source_rgb(cr, 1.0, 0.0, 0.0);
            cs::cairo_fill(cr);
        }

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}