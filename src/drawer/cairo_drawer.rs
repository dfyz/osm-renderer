use errors::*;

use cs;
use geodata::reader::OsmEntities;
use libc;
use mapcss::color::Color;
use mapcss::styler::Styler;
use std::slice;
use tile::{coords_to_float_xy, Tile, TILE_SIZE};

unsafe extern "C" fn write_func(closure: *mut libc::c_void, data: *mut u8, len: libc::c_uint) -> cs::enums::Status {
    let png_bytes: &mut Vec<u8> = &mut *(closure as *mut Vec<u8>);
    png_bytes.extend(slice::from_raw_parts(data, len as usize));
    cs::enums::Status::Success
}

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &Tile, styler: &Styler) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    unsafe {
        let s = cs::cairo_image_surface_create(cs::enums::Format::Rgb24, TILE_SIZE as i32, TILE_SIZE as i32);

        let cr = cs::cairo_create(s);

        let get_delta = |c| -((TILE_SIZE as f64) * (c as f64));
        cs::cairo_translate(cr, get_delta(tile.x), get_delta(tile.y));

        let to_double_color = |u8_color| (u8_color as f64) / 255.0_f64;
        let set_color = |c: &Color| {
            cs::cairo_set_source_rgb(cr, to_double_color(c.r), to_double_color(c.g), to_double_color(c.b));
        };

        if let Some(ref color) = styler.canvas_fill_color {
            set_color(color);
            cs::cairo_paint(cr);
        }

        let all_way_styles = styler.style_ways(entities.ways.iter(), tile.zoom);

        for &(w, ref style) in &all_way_styles {
            if w.node_count() == 0 {
                continue;
            }

            if style.color.is_none() && style.fill_color.is_none() {
                continue;
            }

            let draw_path = || {
                cs::cairo_new_path(cr);

                cs::cairo_set_line_width(cr, style.width.unwrap_or(1.0f64));

                let (x, y) = coords_to_float_xy(&w.get_node(0), tile.zoom);
                cs::cairo_move_to(cr, x, y);
                for i in 1..w.node_count() {
                    let (x, y) = coords_to_float_xy(&w.get_node(i), tile.zoom);
                    cs::cairo_line_to(cr, x, y);
                }
            };

            if let Some(ref c) = style.color {
                draw_path();
                set_color(c);
                cs::cairo_stroke(cr);
            }
        }

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}
