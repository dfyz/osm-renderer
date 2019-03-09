#[derive(Default)]
pub struct PerfStats;
pub struct Measurer;

impl PerfStats {
    pub fn to_html(&self) -> String {
        unimplemented!("This dummy implementation doesn't support HTML rendering")
    }
}

pub fn start_tile(_: u8) {}

pub fn finish_tile(_: &mut PerfStats) {}

pub fn measure(_: impl Into<String>) -> Measurer {
    Measurer {}
}
