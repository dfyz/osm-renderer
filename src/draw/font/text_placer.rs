use draw::figure::Figure;
use draw::font::rasterizer::Rasterizer;
use draw::labelable::Labelable;
use draw::point::Point;
use mapcss::styler::TextPosition;
use stb_truetype::{FontInfo, Vertex, VertexType};
use tile::TILE_SIZE;

pub struct TextPlacer {
    font: FontInfo<&'static [u8]>,
}

impl Default for TextPlacer {
    fn default() -> Self {
        TextPlacer {
            font: FontInfo::new(FONT_DATA, 0).unwrap(),
        }
    }
}

impl TextPlacer {
    pub fn place(
        &self,
        on: &impl Labelable,
        text: &str,
        text_pos: &TextPosition,
        font_size: f64,
        zoom: u8,
        y_offset: usize,
        figure: &mut Figure,
    ) {
        let scale = f64::from(self.font.scale_for_pixel_height(font_size as f32));
        let glyphs = self.text_to_glyphs(text, scale);

        let mut rasterizer = Rasterizer::default();
        let vm = self.get_v_metrics(scale);

        match text_pos {
            TextPosition::Line => if let Some(orig_points) = on.get_waypoints(zoom) {
                let mut points = orig_points.clone();
                if points.len() < 2 {
                    return;
                }
                if points[0].x > points.iter().last().unwrap().x {
                    points.reverse();
                }
                let total_way_length = (1..points.len())
                    .map(|idx| {
                        let from = &points[idx - 1];
                        let to = &points[idx];
                        from.dist(&to)
                    })
                    .sum();

                if glyphs.total_width > total_way_length {
                    return;
                }

                let mut cur_dist = (total_way_length - glyphs.total_width) / 2.0;

                let glyph_center_y = (vm.descent + vm.ascent) / 2.0;
                for glyph in &glyphs.glyphs {
                    let glyph_center_x = glyph.width / 2.0;
                    let way_pos = compute_way_position(&points, cur_dist + glyph_center_x);

                    let tr = |point: &(f64, f64)| {
                        let (original_x, original_y) = point;

                        let translated_x = original_x - glyph_center_x;
                        let translated_y = original_y - glyph_center_y;

                        let (angle_sin, angle_cos) = (-way_pos.angle).sin_cos();

                        let rotated_x = translated_x * angle_cos - translated_y * angle_sin;
                        let rotated_y = translated_y * angle_cos + translated_x * angle_sin;

                        let back_translated_x = way_pos.x + rotated_x;
                        let back_translated_y = way_pos.y - rotated_y;
                        (back_translated_x, back_translated_y)
                    };

                    glyph.rasterize(&mut rasterizer, scale, tr);

                    cur_dist += glyph.width;
                }
            },
            TextPosition::Center => if let Some((center_x, center_y)) = on.get_center(zoom) {
                let mut glyph_rows = Vec::new();
                let mut current_row = Vec::new();
                let mut current_row_width = 0.0;
                let mut max_row_width = 0.0;

                for (idx, glyph) in glyphs.glyphs.iter().enumerate() {
                    current_row.push(glyph);
                    current_row_width += glyph.width;
                    let is_last_glyph = idx + 1 == glyphs.glyphs.len();
                    let should_break = glyph.ch.is_whitespace() && (current_row_width + glyph.width > MAX_TEXT_WIDTH);
                    if !current_row.is_empty() && (should_break || is_last_glyph) {
                        glyph_rows.push((current_row.clone(), current_row_width));
                        if current_row_width > max_row_width {
                            max_row_width = current_row_width;
                        }
                        current_row.clear();
                        current_row_width = 0.0;
                    }
                }

                let row_height = vm.ascent - vm.descent + vm.line_gap;
                let total_height = row_height * glyph_rows.len() as f64;

                let mut cur_y = center_y;
                if y_offset > 0 {
                    cur_y += y_offset as f64;
                } else {
                    cur_y -= total_height / 2.0;
                }

                for (row, row_width) in &glyph_rows {
                    let mut cur_x = center_x - row_width / 2.0;
                    for glyph in row.iter() {
                        let baseline = cur_y + vm.ascent;
                        let x_offset = cur_x;
                        let tr = |point: &(f64, f64)| {
                            let (x, y) = point;
                            (x_offset + x, baseline - y)
                        };
                        glyph.rasterize(&mut rasterizer, scale, tr);
                        cur_x += glyph.width;
                    }
                    cur_y += row_height;
                }
            },
        }

        rasterizer.save_to_figure(figure);
    }

    fn text_to_glyphs(&self, text: &str, scale: f64) -> Glyphs {
        let mut result = Glyphs {
            glyphs: Vec::<Glyph>::default(),
            total_width: 0.0,
        };
        let mut prev_glyph_id: Option<u32> = None;
        for ch in text.chars() {
            let glyph_id = self.font.find_glyph_index(ch as u32);
            let mut advance_width = f64::from(self.font.get_glyph_h_metrics(glyph_id).advance_width);

            let mut glyph = Glyph {
                ch,
                width: advance_width * scale,
                shape: self.font.get_glyph_shape(glyph_id),
            };

            if let Some(prev_glyph) = prev_glyph_id {
                let kern_advance = f64::from(self.font.get_glyph_kern_advance(prev_glyph, glyph_id));
                glyph.width += kern_advance * scale;
            }

            result.total_width += glyph.width;
            prev_glyph_id = Some(glyph_id);

            result.glyphs.push(glyph);
        }
        result
    }

    fn get_v_metrics(&self, scale: f64) -> VMetrics {
        let convert = |x| f64::from(x) * scale;
        let vm = self.font.get_v_metrics();
        VMetrics {
            descent: convert(vm.descent),
            ascent: convert(vm.ascent),
            line_gap: convert(vm.line_gap),
        }
    }
}

struct VMetrics {
    descent: f64,
    ascent: f64,
    line_gap: f64,
}

struct Glyph {
    ch: char,
    width: f64,
    shape: Option<Vec<Vertex>>,
}

impl Glyph {
    fn rasterize<F>(&self, rasterizer: &mut Rasterizer, scale: f64, tr: F)
    where
        F: Fn(&(f64, f64)) -> (f64, f64),
    {
        let convert = |x, y| (f64::from(x) * scale, f64::from(y) * scale);

        if let Some(ref vertices) = self.shape {
            let mut from = (0.0, 0.0);
            for v in vertices {
                let to = convert(v.x, v.y);
                match v.vertex_type() {
                    VertexType::MoveTo => {}
                    VertexType::LineTo => {
                        let (p1, p0) = (tr(&from), tr(&to));
                        rasterizer.draw_line(p0.0, p0.1, p1.0, p1.1);
                    }
                    VertexType::CurveTo => {
                        let midpoint = convert(v.cx, v.cy);
                        let (p2, p1, p0) = (tr(&from), tr(&midpoint), tr(&to));
                        rasterizer.draw_quad(p0.0, p0.1, p1.0, p1.1, p2.0, p2.1);
                    }
                }
                from = to;
            }
        }
    }
}

struct Glyphs {
    glyphs: Vec<Glyph>,
    total_width: f64,
}

fn get_angle(points: &[Point], start_idx: usize) -> f64 {
    let from = &points[start_idx];
    let to = &points[start_idx + 1];
    let x = f64::from(to.x - from.x);
    let y = f64::from(to.y - from.y);
    y.atan2(x)
}

struct WayPosition {
    x: f64,
    y: f64,
    angle: f64,
}

fn compute_way_position(points: &[Point], advance_by: f64) -> WayPosition {
    let mut point_idx = 0;
    let mut to_travel = advance_by;
    while to_travel > 0.0 && point_idx + 1 < points.len() {
        let seg_dist = points[point_idx].dist(&points[point_idx + 1]);
        if seg_dist >= to_travel {
            let from = &points[point_idx];
            let to = &points[point_idx + 1];
            let ratio = to_travel / from.dist(&to);
            let coord_dist = |from_c, to_c| (f64::from(from_c) + (f64::from(to_c - from_c) * ratio));
            return WayPosition {
                x: coord_dist(from.x, to.x),
                y: coord_dist(from.y, to.y),
                angle: get_angle(points, point_idx),
            };
        } else {
            to_travel -= seg_dist;
            point_idx += 1;
        }
    }
    let last_point = points.iter().last().unwrap();
    WayPosition {
        x: f64::from(last_point.x),
        y: f64::from(last_point.y),
        angle: get_angle(points, points.len() - 2),
    }
}

const MAX_TEXT_WIDTH: f64 = TILE_SIZE as f64 / 8.0;
const FONT_DATA: &[u8] = include_bytes!("NotoSans-Regular.ttf");
