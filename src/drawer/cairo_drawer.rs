use errors::*;

use cs;
use geodata::reader::{OsmEntities, Way};
use libc;
use mapcss::color::Color;
use mapcss::styler::{LineCap, LineJoin, Style, Styler};
use std::slice;
use tile::{coords_to_float_xy, Tile, TILE_SIZE};

unsafe extern "C" fn write_func(closure: *mut libc::c_void, data: *mut u8, len: libc::c_uint) -> cs::enums::Status {
    let png_bytes: &mut Vec<u8> = &mut *(closure as *mut Vec<u8>);
    png_bytes.extend(slice::from_raw_parts(data, len as usize));
    cs::enums::Status::Success
}

unsafe fn set_color(cr: *mut cs::cairo_t, color: &Color, opacity: f64) {
    let to_double_color = |u8_color| (u8_color as f64) / 255.0_f64;
    cs::cairo_set_source_rgba(cr, to_double_color(color.r), to_double_color(color.g), to_double_color(color.b), opacity);
}

unsafe fn draw_way_path(cr: *mut cs::cairo_t, w: &Way, style: &Style, zoom: u8) {
    cs::cairo_set_line_width(cr, style.width.unwrap_or(1.0f64));

    let cairo_line_cap = match style.line_cap {
        Some(LineCap::Round) => cs::enums::LineCap::Round,
        Some(LineCap::Square) => cs::enums::LineCap::Square,
        _ => cs::enums::LineCap::Butt,
    };
    cs::cairo_set_line_cap(cr, cairo_line_cap);

    let cairo_line_join = match style.line_join {
        Some(LineJoin::Bevel) => cs::enums::LineJoin::Bevel,
        Some(LineJoin::Miter) => cs::enums::LineJoin::Miter,
        _ => cs::enums::LineJoin::Round,
    };
    cs::cairo_set_line_join(cr, cairo_line_join);

    cs::cairo_new_path(cr);
    let (x, y) = coords_to_float_xy(&w.get_node(0), zoom);
    cs::cairo_move_to(cr, x + 0.5, y + 0.5);
    for i in 1..w.node_count() {
        let (x, y) = coords_to_float_xy(&w.get_node(i), zoom);
        cs::cairo_line_to(cr, x + 0.5, y + 0.5);
    }
}

unsafe fn draw_way(cr: *mut cs::cairo_t, w: &Way, style: &Style, zoom: u8) {
    let default_dashes = Vec::new();
    let dashes = style.dashes.as_ref().unwrap_or(&default_dashes);
    cs::cairo_set_dash(cr, dashes.as_ptr(), dashes.len() as i32, 0.0);

    if let Some(ref c) = style.color {
        draw_way_path(cr, w, style, zoom);
        set_color(cr, c, style.opacity.unwrap_or(1.0));
        cs::cairo_stroke(cr);
    }
}

unsafe fn fill_way<'a>(cr: *mut cs::cairo_t, w: &Way<'a>, style: &Style, zoom: u8) {
    if let Some(ref c) = style.fill_color {
        draw_way_path(cr, w, style, zoom);
        set_color(cr, c, style.fill_opacity.unwrap_or(1.0));
        cs::cairo_fill(cr);
    }
}

pub fn draw_tile<'a>(entities: &OsmEntities<'a>, tile: &Tile, styler: &Styler) -> Result<Vec<u8>> {
    let mut data = Vec::new();

    unsafe {
        let s = cs::cairo_image_surface_create(cs::enums::Format::Rgb24, TILE_SIZE as i32, TILE_SIZE as i32);

        let cr = cs::cairo_create(s);

        let get_delta = |c| -((TILE_SIZE as f64) * (c as f64));
        cs::cairo_translate(cr, get_delta(tile.x), get_delta(tile.y));

        if let Some(ref color) = styler.canvas_fill_color {
            set_color(cr, color, 1.0);
            cs::cairo_paint(cr);
        }

        let all_way_styles = styler.style_ways(entities.ways.iter(), tile.zoom);

        for &(w, ref style) in &all_way_styles {
            fill_way(cr, w, style, tile.zoom);
        }

        for &(w, ref style) in &all_way_styles {
            draw_way(cr, w, style, tile.zoom);
        }

        cs::cairo_destroy(cr);

        cs::cairo_surface_write_to_png_stream(s, Some(write_func), &mut data as *mut Vec<u8> as *mut libc::c_void);
        cs::cairo_surface_destroy(s);
    }

    Ok(data)
}
