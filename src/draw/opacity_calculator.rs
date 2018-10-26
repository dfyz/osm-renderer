use mapcss::styler::{is_non_trivial_cap, LineCap};
use std::cmp::Ordering;

pub struct OpacityCalculator {
    half_line_width: f64,
    dashes: Vec<DashSegment>,
    total_dash_len: f64,
    traveled_distance: f64,
}

pub struct OpacityData {
    pub opacity: f64,
    pub is_in_line: bool,
}

impl OpacityCalculator {
    pub fn new(half_line_width: f64, dashes: &Option<Vec<f64>>, line_cap: &Option<LineCap>) -> Self {
        let mut dash_segments = Vec::new();
        let mut len_before = 0.0;

        if let Some(ref dashes) = *dashes {
            compute_segments(half_line_width, dashes, line_cap, &mut dash_segments, &mut len_before);
        }

        Self {
            half_line_width,
            dashes: dash_segments,
            total_dash_len: len_before,
            traveled_distance: 0.0,
        }
    }

    pub fn calculate(&self, center_distance: f64, start_distance: f64) -> OpacityData {
        let sd = self.get_opacity_by_start_distance(start_distance);

        let cap_dist = sd.distance_in_cap.unwrap_or_default();
        let half_line_width = (self.half_line_width.powi(2) - cap_dist.powi(2)).sqrt();

        let cd = get_opacity_by_center_distance(center_distance, half_line_width);
        OpacityData {
            opacity: sd.opacity.min(cd),
            is_in_line: cd > 0.0,
        }
    }

    pub fn add_traveled_distance(&mut self, distance: f64) {
        self.traveled_distance += distance;
    }

    fn get_opacity_by_start_distance(&self, start_distance: f64) -> StartDistanceOpacityData {
        if self.dashes.is_empty() {
            return StartDistanceOpacityData {
                opacity: 1.0,
                distance_in_cap: None,
            };
        }

        let mut dist_rem = self.traveled_distance + start_distance;
        if self.total_dash_len > 0.0 {
            dist_rem %= self.total_dash_len;
        }
        let safe_cmp_floats = |x: &f64, y: &f64| x.partial_cmp(y).unwrap_or(Ordering::Equal);
        let opacities_with_cap_distances = self
            .dashes
            .iter()
            .filter_map(|d| get_opacity_by_segment(dist_rem, d).map(|op| (op, get_distance_in_cap(dist_rem, d))))
            .collect::<Vec<_>>();

        StartDistanceOpacityData {
            opacity: opacities_with_cap_distances
                .iter()
                .map(|x| x.0)
                .max_by(&safe_cmp_floats)
                .unwrap_or_default(),
            distance_in_cap: opacities_with_cap_distances
                .iter()
                .filter_map(|x| x.1)
                .min_by(&safe_cmp_floats),
        }
    }
}

struct StartDistanceOpacityData {
    opacity: f64,
    distance_in_cap: Option<f64>,
}

#[derive(Debug)]
struct DashSegment {
    start_from: f64,
    start_to: f64,
    end_from: f64,
    end_to: f64,
    opacity_mul: f64,
    original_endpoints: Option<(f64, f64)>,
}

fn compute_segments(
    half_line_width: f64,
    dashes: &[f64],
    line_cap: &Option<LineCap>,
    segments: &mut Vec<DashSegment>,
    len_before: &mut f64,
) {
    // Use the first dash twice to make sure we don't miss the very first cap.
    let dash_indexes = (0..dashes.len()).chain(0..1);

    for idx in dash_indexes {
        let dash = dashes[idx];
        let mut start = *len_before;

        if idx != 0 || segments.is_empty() {
            *len_before += dash;
        }

        if idx % 2 != 0 {
            continue;
        }

        let mut end = start + dash;

        let original_endpoints = match *line_cap {
            Some(LineCap::Round) => Some((start, end)),
            _ => None,
        };

        if is_non_trivial_cap(line_cap) {
            start -= half_line_width;
            end += half_line_width;
        }

        let midpoint = (start + end) / 2.0;

        segments.push(DashSegment {
            start_from: (start - 0.5).min(midpoint - 1.0),
            start_to: (start + 0.5).min(midpoint),
            end_from: (end - 0.5).max(midpoint),
            end_to: (end + 0.5).max(midpoint + 1.0),
            opacity_mul: (end - start).min(1.0),
            original_endpoints,
        })
    }
}

fn get_opacity_by_segment(dist: f64, segment: &DashSegment) -> Option<f64> {
    let base_opacity = if dist < segment.start_from || dist > segment.end_to {
        None
    } else if dist <= segment.start_to {
        Some((dist - segment.start_from) / (segment.start_to - segment.start_from))
    } else if dist < segment.end_from {
        Some(1.0)
    } else {
        Some((segment.end_to - dist) / (segment.end_to - segment.end_from))
    };

    base_opacity.map(|op| segment.opacity_mul * op)
}

fn get_distance_in_cap(dist: f64, segment: &DashSegment) -> Option<f64> {
    segment.original_endpoints.map(|(a, b)| {
        if dist < a {
            a - dist
        } else if dist <= b {
            0.0
        } else {
            dist - b
        }
    })
}

fn get_opacity_by_center_distance(center_distance: f64, half_line_width: f64) -> f64 {
    let feather_from = (half_line_width - 0.5).max(0.0);
    let feather_to = (half_line_width + 0.5).max(1.0);
    let feather_dist = feather_to - feather_from;
    let opacity_mul = (2.0 * half_line_width).min(1.0);

    opacity_mul
        * (if center_distance < feather_from {
            1.0
        } else if center_distance < feather_to {
            (feather_to - center_distance) / feather_dist
        } else {
            0.0
        })
}
