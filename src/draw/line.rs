use draw::figure::Figure;
use draw::opacity_calculator::OpacityCalculator;
use draw::point::Point;
use draw::point_pairs::PointPairIter;
use draw::tile_pixels::RgbaColor;
use mapcss::color::Color;
use mapcss::styler::{is_non_trivial_cap, LineCap};

pub fn draw_lines(
    points: PointPairIter,
    width: f64,
    color: &Color,
    opacity: f64,
    dashes: &Option<Vec<f64>>,
    line_cap: &Option<LineCap>,
    use_caps_for_dashes: bool,
    figure: &mut Figure,
) {
    let half_width = width / 2.0;
    let line_cap_for_dashes = if use_caps_for_dashes { line_cap } else { &None };
    let mut opacity_calculator = OpacityCalculator::new(half_width, dashes, line_cap_for_dashes);
    let opacity_calculator_for_outer_caps = OpacityCalculator::new(half_width, &Some(vec![0.0]), line_cap);

    let has_caps = is_non_trivial_cap(line_cap);

    let mut peekable_points = points.peekable();
    let mut first = true;

    while let Some((p1, p2)) = peekable_points.next() {
        draw_line(&p1, &p2, color, opacity, &opacity_calculator, figure);
        opacity_calculator.add_traveled_distance(p1.dist(&p2));

        if p1 != p2 && has_caps {
            if first {
                let cap_end = p1.push_away_from(&p2, half_width);
                draw_line(
                    &p1,
                    &cap_end,
                    color,
                    opacity,
                    &opacity_calculator_for_outer_caps,
                    figure,
                );
            }

            if peekable_points.peek().is_none() {
                let cap_end = p2.push_away_from(&p1, half_width);
                draw_line(
                    &p2,
                    &cap_end,
                    color,
                    opacity,
                    &opacity_calculator_for_outer_caps,
                    figure,
                );
            }
        }

        first = false;
    }
}

// Full-blown Bresenham with anti-aliasing and thick line support.
// Mostly inspired by http://kt8216.unixcab.org/murphy/index.html
fn draw_line(
    p1: &Point,
    p2: &Point,
    color: &Color,
    initial_opacity: f64,
    opacity_calculator: &OpacityCalculator,
    figure: &mut Figure,
) {
    if p1 == p2 {
        return;
    }

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

    let center_dist_numer_const = (i64::from(p2.x) * i64::from(p1.y)) - (i64::from(p2.y) * i64::from(p1.x));
    let (sdx, sdy) = (i64::from(p2.x) - i64::from(p1.x), i64::from(p2.y) - i64::from(p1.y));
    let (dx_float, dy_float) = (f64::from(dx), f64::from(dy));
    let center_dist_denom = (dy_float * dy_float + dx_float * dx_float).sqrt();

    let mut draw_perpendiculars = |mn, mx, p_error| {
        let mut draw_one_perpendicular = |mul| {
            let mut p_mn = mx;
            let mut p_mx = mn;
            let mut error = mul * p_error;
            loop {
                let (perp_x, perp_y) = swap_x_y_if_needed(p_mx, p_mn, should_swap_x_y);
                let current_point = Point { x: perp_x, y: perp_y };

                let center_dist_numer_non_const = sdy * i64::from(perp_x) - sdx * i64::from(perp_y);
                let center_dist_raw = center_dist_numer_const + center_dist_numer_non_const;
                let center_dist = (center_dist_raw as f64).abs() / center_dist_denom;

                let long_start_dist = current_point.dist(p1);
                let short_start_dist = (long_start_dist.powi(2) - center_dist.powi(2)).max(0.0).sqrt();

                let opacity_params = opacity_calculator.calculate(center_dist, short_start_dist);

                if !opacity_params.is_in_line {
                    break;
                }

                let current_color = RgbaColor::from_color(color, initial_opacity * opacity_params.opacity);
                figure.add(current_point.x as usize, current_point.y as usize, current_color);

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

fn swap_x_y_if_needed<T>(a: T, b: T, should_swap: bool) -> (T, T) {
    if should_swap {
        (b, a)
    } else {
        (a, b)
    }
}
