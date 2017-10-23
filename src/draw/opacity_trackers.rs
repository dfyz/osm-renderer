pub struct CenterDistanceOpacityTracker {
    feather_from: f64,
    feather_to: f64,
    feather_dist: f64,
    opacity_mul: f64,
}

pub struct StartDistanceOpacityTracker {
    dashes: Vec<DashSegment>,
    total_dash_len: f64,
    traveled_distance: f64,
}

impl CenterDistanceOpacityTracker {
    pub fn new(line_width: f64) -> Self {
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

    pub fn get_opacity(&self, center_distance: f64) -> f64 {
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

impl StartDistanceOpacityTracker {
    pub fn new(dashes: &Option<Vec<f64>>) -> Self {
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

    pub fn get_opacity(&self, start_distance: f64) -> f64 {
        if self.dashes.is_empty() {
            return 1.0;
        }
        let dist_rem = (self.traveled_distance + start_distance) % self.total_dash_len;
        for dash in &self.dashes {
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

    pub fn add_traveled_distance(&mut self, distance: f64) {
        self.traveled_distance += distance;
    }
}
