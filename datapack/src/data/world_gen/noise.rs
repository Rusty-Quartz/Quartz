use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Noise {
    #[serde(rename = "firstOctave")]
    pub first_octave: i32,
    pub amplitudes: Vec<f64>,
}
