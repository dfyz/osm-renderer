pub trait Coords {
    fn lat(&self) -> f64;
    fn lon(&self) -> f64;
}

impl Coords for (f64, f64) {
    fn lat(&self) -> f64 {
        self.0
    }

    fn lon(&self) -> f64 {
        self.1
    }
}
