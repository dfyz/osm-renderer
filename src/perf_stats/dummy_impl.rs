#[derive(Default)]
pub struct PerfStats;
pub struct Measurer;

pub fn start_tile(_: u8) {
}

pub fn finish_tile(_: &mut PerfStats) {
}

pub fn measure(_: impl Into<String>) -> Measurer {
    Measurer {}
}
