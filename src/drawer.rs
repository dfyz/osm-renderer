use errors::*;

use cs;
use geodata::reader::OsmEntities;
use libc;
use std::f64::consts::PI;
use std::slice;
use tile::{coords_to_float_xy, Tile, TILE_SIZE};

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

        let get_delta = |c| -((TILE_SIZE as f64) * (c as f64));
        cs::cairo_translate(cr, get_delta(tile.x), get_delta(tile.y));

        for w in entities.ways.iter() {
            if w.node_count() == 0 {
                continue;
            }
            cs::cairo_set_source_rgb(cr, 0.0, 1.0, 0.0);
            let (x, y) = coords_to_float_xy(&w.get_node(0), tile.zoom);
            cs::cairo_move_to(cr, x, y);
            for i in 1..w.node_count() {
                let (x, y) = coords_to_float_xy(&w.get_node(i), tile.zoom);
                cs::cairo_line_to(cr, x, y);
            }
            cs::cairo_stroke(cr);
        }

        for n in entities.nodes.iter() {
            let (x, y) = coords_to_float_xy(n, tile.zoom);
            cs::cairo_arc(cr, x, y, 1.0, 0.0, 2.0*PI);
            cs::cairo_set_source_rgb(cr, 1.0, 0.0, 0.0);
            cs::cairo_fill(cr);
        }

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}
