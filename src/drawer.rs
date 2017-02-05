use errors::*;

use cs;
use geodata::reader::OsmEntities;
use libc;
use std::{ffi, slice};
use tile::{Tile, TILE_SIZE};

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

        cs::cairo_set_source_rgb(cr, 1.0, 0.0, 0.0);
        cs::cairo_set_font_size(cr, 48.0);
        let text = ffi::CString::new("джигурда").unwrap();

        cs::cairo_move_to(cr, 10.0, 50.0);
        cs::cairo_show_text(cr, text.as_ptr() as *const libc::c_char);

        let font_name = ffi::CString::new("Comic Sans MS").unwrap();
        cs::cairo_select_font_face(cr, font_name.as_ptr() as *const libc::c_char, cs::enums::FontSlant::Normal, cs::enums::FontWeight::Normal);
        cs::cairo_move_to(cr, 10.0, 100.0);
        cs::cairo_show_text(cr, text.as_ptr() as *const libc::c_char);

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}