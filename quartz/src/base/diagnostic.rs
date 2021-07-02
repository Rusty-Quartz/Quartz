pub struct Diagnostics {
    pub(crate) microseconds_per_tick: f64,
}

impl Diagnostics {
    pub const fn new() -> Self {
        Diagnostics {
            microseconds_per_tick: 0.0,
        }
    }

    /// Returns a buffered milliseconds per tick (MSPT) measurement. This reading is buffer for 100
    /// tick cycles.
    #[inline]
    pub fn mspt(&self) -> f64 {
        self.microseconds_per_tick / 1000.0
    }
}
