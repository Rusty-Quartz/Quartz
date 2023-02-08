use quartz_util::math::LerpExt;

pub mod blended;
pub mod normal;
pub mod perlin;
pub mod simplex;

// Utility math functions used in noise

pub struct NoiseSamplingSettings {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
}
