use std::collections::BTreeMap;

use crate::draw::figure::Figure;
use crate::draw::tile_pixels::RgbaColor;
use crate::mapcss::color::Color;

#[derive(Default)]
struct Stripe {
    a: BTreeMap<i32, f64>,
    s: BTreeMap<i32, f64>,
}

type Stripes = BTreeMap<i32, Stripe>;

pub struct Rasterizer {
    stripes: Stripes,
    color: Color,
}

impl Rasterizer {
    pub fn new(color: &Color) -> Rasterizer {
        Rasterizer {
            stripes: Stripes::default(),
            color: color.clone(),
        }
    }

    pub fn draw_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        let delta = y1 - y0;

        if delta == 0.0 {
            return;
        }

        let sign = if y0 <= y1 { 1.0 } else { -1.0 };

        let slope = (x1 - x0) / delta;
        let eval_x_at_y = |y| x0 + (y - y0) * slope;
        let eval_y_at_x = |x| y0 + (x - x0) * slope.recip();

        let y_min = y0.min(y1);
        let y_max = y0.max(y1);

        for y in (y_min.floor() as i32)..=(y_max.floor() as i32) {
            let current_stripes = self.stripes.entry(y).or_insert_with(Default::default);

            let y_bottom = f64::from(y).max(y_min);
            let y_top = f64::from(y + 1).min(y_max);
            let y_delta = y_top - y_bottom;

            let x_at_bottom = eval_x_at_y(y_bottom);
            let x_at_top = eval_x_at_y(y_top);

            let (flip_edge, x_smallest, x_largest) = if x_at_bottom <= x_at_top {
                (false, x_at_bottom, x_at_top)
            } else {
                (true, x_at_top, x_at_bottom)
            };

            let x_to = x_largest.floor() as i32;
            for x in (x_smallest.floor() as i32)..=x_to {
                let x_left = f64::from(x).max(x_smallest);
                let x_next = f64::from(x + 1) as f64;
                let x_right = x_next.min(x_largest);

                let mut pixel_area = (x_next - x_right) * y_delta;
                let trapezoid_width = x_right - x_left;
                if trapezoid_width > 0.0 {
                    let y_at_left = eval_y_at_x(x_left);
                    let y_at_right = eval_y_at_x(x_right);

                    let trapezoid_height = if flip_edge {
                        (y_top - y_at_left) + (y_top - y_at_right)
                    } else {
                        (y_at_left - y_bottom) + (y_at_right - y_bottom)
                    };

                    pixel_area += trapezoid_width * trapezoid_height / 2.0;
                }
                *current_stripes.a.entry(x).or_insert(0.0) += sign * pixel_area;
            }

            *current_stripes.s.entry(x_to + 1).or_insert(0.0) += sign * y_delta;
        }
    }

    pub fn draw_quad(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) {
        let dist_between = |xa: f64, ya: f64, xb: f64, yb: f64| (xa - xb).abs().hypot((ya - yb).abs());
        let d01 = dist_between(x0, y0, x1, y1);
        let d12 = dist_between(x1, y1, x2, y2);
        let d02 = dist_between(x0, y0, x2, y2);

        if (d01 + d12) <= 1.0001 * d02 {
            self.draw_line(x0, y0, x2, y2);
            return;
        }

        let midpoint = |c1, c2| (c1 + c2) / 2.0;
        let m01_x = midpoint(x0, x1);
        let m01_y = midpoint(y0, y1);
        let m12_x = midpoint(x1, x2);
        let m12_y = midpoint(y1, y2);
        let m012_x = midpoint(m01_x, m12_x);
        let m012_y = midpoint(m01_y, m12_y);

        self.draw_quad(x0, y0, m01_x, m01_y, m012_x, m012_y);
        self.draw_quad(m012_x, m012_y, m12_x, m12_y, x2, y2);
    }

    pub fn save_to_figure(&self, figure: &mut Figure) {
        let mut x_min = i32::max_value();
        let mut x_max = i32::min_value();
        for stripe in self.stripes.values() {
            if !stripe.a.is_empty() {
                x_min = x_min.min(*stripe.a.keys().min().unwrap());
                x_max = x_max.max(*stripe.a.keys().max().unwrap());
            }
            if !stripe.s.is_empty() {
                x_min = x_min.min(*stripe.s.keys().min().unwrap());
                x_max = x_max.max(*stripe.s.keys().max().unwrap());
            }
        }

        for (y, stripe) in &self.stripes {
            let cur_a = stripe.a.iter().collect();
            let cur_s = stripe.s.iter().collect();
            let mut a_idx = 0;
            let mut s_idx = 0;
            let mut s_acc = 0.0;

            let extract_val = |vec: &Vec<(&i32, &f64)>, idx: &mut usize, x| {
                if *idx < vec.len() && *vec[*idx].0 == x {
                    let val = *vec[*idx].1;
                    *idx += 1;
                    val
                } else {
                    0.0
                }
            };

            for x in x_min..=x_max {
                s_acc += extract_val(&cur_s, &mut s_idx, x);
                let total = extract_val(&cur_a, &mut a_idx, x) + s_acc;
                figure.add(x as usize, *y as usize, RgbaColor::from_color(&self.color, total));
            }
        }
    }
}
