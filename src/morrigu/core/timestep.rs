#[derive(Clone)]
pub struct Timestep {
    nanoseconds: u128,
}

impl Timestep {
    pub fn from_nano(nanoseconds: u128) -> Timestep {
        Timestep { nanoseconds }
    }

    pub fn as_seconds(&self) -> f64 {
        self.nanoseconds as f64 / 1000000000f64
    }
}
