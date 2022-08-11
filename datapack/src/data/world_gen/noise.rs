use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Noise {
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}
