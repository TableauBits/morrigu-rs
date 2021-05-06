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

    pub fn as_millis(&self) -> f64 {
        self.nanoseconds as f64 / 1000000f64
    }
}

impl std::fmt::Display for Timestep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}ms", self.as_millis())
    }
}
