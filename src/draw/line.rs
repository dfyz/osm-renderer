use draw::figure::Figure;
use draw::png_image::RgbaColor;
use draw::point::Point;
use mapcss::color::Color;

pub fn draw_lines<I>(points: I, width: f64, color: &Color, opacity: f64, dashes: &Option<Vec<f64>>) -> Figure
    where I: Iterator<Item=(Point, Point)>
{
    let mut figure = Default::default();
    let mut sd_tracker = StartDistanceOpacityTracker::new(dashes);
    for (p1, p2) in points {
        draw_line(&p1, &p2, width, color, opacity, &sd_tracker, &mut figure);
        sd_tracker.add_traveled_distance(p1.dist(&p2));
    }

    figure
}

// Full-blown Bresenham with anti-aliasing and thick line support.
// Mostly inspired by http://kt8216.unixcab.org/murphy/index.html
fn draw_line(
    p1: &Point,
    p2: &Point,
    width: f64,
    color: &Color,
    opacity: f64,
    sd_tracker: &StartDistanceOpacityTracker,
    figure: &mut Figure
) {
    let get_inc = |from, to| if from <= to { 1 } else { -1 };

    let (dx, dy) = ((p2.x - p1.x).abs(), (p2.y - p1.y).abs());
    let (mut x0, mut y0) = (p1.x, p1.y);
    let should_swap_x_y = dx > dy;

    let (mn, mx) = swap_x_y_if_needed(&mut x0, &mut y0, should_swap_x_y);
    let (mn_last, mx_last) = swap_x_y_if_needed(p2.x, p2.y, should_swap_x_y);
    let (mn_delta, mx_delta) = swap_x_y_if_needed(dx, dy, should_swap_x_y);
    let (mn_inc, mx_inc) = swap_x_y_if_needed(get_inc(p1.x, p2.x), get_inc(p1.y, p2.y), should_swap_x_y);

    let mut error = 0;
    let mut p_error = 0;

    let update_error = |error: &mut i32| {
        let was_corrected = if *error + 2 * mn_delta > mx_delta {
            *error -= 2 * mx_delta;
            true
        } else {
            false
        };
        *error += 2 * mn_delta;
        was_corrected
    };

    let center_dist_numer_const = f64::from((p2.x * p1.y) - (p2.y * p1.x));
    let center_dist_denom = (f64::from(dy*dy + dx*dx)).sqrt();

    let cd_tracker = CenterDistanceOpacityTracker::new(width);

    let mut draw_perpendiculars = |mn, mx, p_error| {
        let mut draw_one_perpendicular = |mul| {
            let mut p_mn = mx;
            let mut p_mx = mn;
            let mut error = mul * p_error;
            loop {
                let (perp_x, perp_y) = swap_x_y_if_needed(p_mx, p_mn, should_swap_x_y);
                let current_point = Point {
                    x: perp_x,
                    y: perp_y,
                };

                let center_dist_numer_non_const = f64::from((p2.y - p1.y) * perp_x - (p2.x - p1.x) * perp_y);
                let center_dist = (center_dist_numer_const + center_dist_numer_non_const).abs() / center_dist_denom;

                let cd_opacity = cd_tracker.get_opacity(center_dist);

                if cd_opacity <= 0.0 {
                    break;
                }

                let long_start_dist = current_point.dist(p1);
                let short_start_dist = (long_start_dist.powi(2) - center_dist.powi(2)).sqrt();

                let sd_opacity = sd_tracker.get_opacity(short_start_dist);
                let final_opacity_mul = cd_opacity.min(sd_opacity);

                if final_opacity_mul > 0.0 {
                    let current_color = RgbaColor::from_color(color, opacity * final_opacity_mul);
                    figure.add(current_point.x as usize, current_point.y as usize, current_color);
                }

                if update_error(&mut error) {
                    p_mn -= mul * mx_inc;
                }
                p_mx += mul * mn_inc;
            }
        };

        draw_one_perpendicular(1);
        draw_one_perpendicular(-1);
    };

    loop {
        draw_perpendiculars(*mn, *mx, p_error);

        if *mn == mn_last && *mx == mx_last {
            break;
        }

        if update_error(&mut error) {
            *mn += mn_inc;
            if update_error(&mut p_error) {
                draw_perpendiculars(*mn, *mx, p_error);
            }
        }
        *mx += mx_inc;
    }
}

struct CenterDistanceOpacityTracker {
    feather_from: f64,
    feather_to: f64,
    feather_dist: f64,
    opacity_mul: f64,
}

impl CenterDistanceOpacityTracker {
    fn new(line_width: f64) -> Self {
        let line_half_width = line_width / 2.0;
        let feather_from = (line_half_width - 0.5).max(0.0);
        let feather_to = (line_half_width + 0.5).max(1.0);
        let feather_dist = feather_to - feather_from;
        CenterDistanceOpacityTracker {
            feather_from,
            feather_to,
            feather_dist,
            opacity_mul: line_width.min(1.0),
        }
    }

    fn get_opacity(&self, center_distance: f64) -> f64 {
        self.opacity_mul * (if center_distance < self.feather_from {
            1.0
        } else if center_distance < self.feather_to {
            (self.feather_to - center_distance) / self.feather_dist
        } else {
            0.0
        })
    }
}

struct DashSegment {
    start_from: f64,
    start_to: f64,
    end_from: f64,
    end_to: f64,
    opacity_mul: f64,
}

struct StartDistanceOpacityTracker {
    dashes: Vec<DashSegment>,
    total_dash_len: f64,
    traveled_distance: f64,
}

impl StartDistanceOpacityTracker {
    fn new(dashes: &Option<Vec<f64>>) -> Self {
        let mut dash_segments = Vec::new();
        let mut len_before = 0.0;

        if let Some(ref dashes) = *dashes {
            for (idx, dash) in dashes.iter().enumerate() {
                if idx % 2 == 0 {
                    let start = len_before;
                    let end = len_before + dash;

                    let midpoint = (start + end) / 2.0;

                    dash_segments.push(DashSegment {
                        start_from: (start - 0.5).min(midpoint - 1.0),
                        start_to: (start + 0.5).min(midpoint),
                        end_from: (end - 0.5).max(midpoint),
                        end_to: (end + 0.5).max(midpoint + 1.0),
                        opacity_mul: (end - start).min(1.0),
                    })
                }
                len_before += *dash;
            }
        }

        Self {
            dashes: dash_segments,
            total_dash_len: len_before,
            traveled_distance: 0.0,
        }
    }

    fn get_opacity(&self, start_distance: f64) -> f64 {
        if self.dashes.is_empty() {
            return 1.0;
        }
        let dist_rem = (self.traveled_distance + start_distance) % self.total_dash_len;
        for dash in self.dashes.iter() {
            let mul = dash.opacity_mul;
            if dist_rem < dash.start_from {
                return 0.0;
            } else if dist_rem <= dash.start_to {
                return mul * ((dist_rem - dash.start_from) / (dash.start_to - dash.start_from));
            } else if dist_rem < dash.end_from {
                return mul;
            } else if dist_rem <= dash.end_to {
                return mul * ((dash.end_to - dist_rem) / (dash.end_to - dash.end_from));
            }
        }

        0.0
    }

    fn add_traveled_distance(&mut self, distance: f64) {
        self.traveled_distance += distance;
    }
}

fn swap_x_y_if_needed<T>(a: T, b: T, should_swap: bool) -> (T, T) {
    if should_swap {
        (b, a)
    } else {
        (a, b)
    }
}
