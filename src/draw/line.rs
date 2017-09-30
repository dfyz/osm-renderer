use draw::figure::Figure;
use draw::png_image::RgbaColor;
use draw::point::Point;
use mapcss::color::Color;

pub fn draw_lines<I>(points: I, width: f64, color: &Color, opacity: f64) -> Figure
    where I: Iterator<Item=(Point, Point)>
{
    let mut figure = Default::default();

    for (p1, p2) in points {
        draw_line(&p1, &p2, width, color, opacity, &mut figure);
    }

    figure
}

// Full-blown Bresenham with anti-aliasing and thick line support.
// Mostly inspired by http://kt8216.unixcab.org/murphy/index.html
pub fn draw_line(p1: &Point, p2: &Point, width: f64, color: &Color, opacity: f64, figure: &mut Figure) {
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

                let center_dist_numer_non_const = f64::from((p2.y - p1.y) * perp_x - (p2.x - p1.x) * perp_y);
                let center_dist = (center_dist_numer_const + center_dist_numer_non_const).abs() / center_dist_denom;

                let cd_opacity = cd_tracker.get_opacity(center_dist);

                if cd_opacity <= 0.0 {
                    break;
                }

                figure.add(perp_x as usize, perp_y as usize, RgbaColor::from_color(color, opacity * cd_opacity));

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
        if center_distance < self.feather_from {
            self.opacity_mul
        } else if center_distance < self.feather_to {
            (self.feather_to - center_distance) / self.feather_dist * self.opacity_mul
        } else {
            0.0
        }
    }
}

fn swap_x_y_if_needed<T>(a: T, b: T, should_swap: bool) -> (T, T) {
    if should_swap {
        (b, a)
    } else {
        (a, b)
    }
}
