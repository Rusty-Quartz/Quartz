pub mod blended;
pub mod normal;
pub mod perlin;
pub mod simplex;

// Utility math functions used in noise
// maybe move these into util later?

/// Returns the dot product of the provided gradient vector and the vector (x, y, z)
pub(self) fn dot(gradient: [i32; 3], x: f64, y: f64, z: f64) -> f64 {
    gradient[0] as f64 * x + gradient[1] as f64 * y + gradient[2] as f64 * z
}

pub(self) fn smooth_step(val: f64) -> f64 {
    val * val * val * (val * (val * 6.0 - 15.0) + 10.0)
}

pub(self) fn lerp(delta: f32, start: f32, end: f32) -> f32 {
    start + delta * (end - start)
}

pub(self) fn lerp_2d(
    delta_x: f64,
    delta_y: f64,
    x0y0: f64,
    x1y0: f64,
    x0y1: f64,
    x1y1: f64,
) -> f64 {
    lerp(
        delta_y as f32,
        lerp(delta_x as f32, x0y0 as f32, x1y0 as f32),
        lerp(delta_x as f32, x0y1 as f32, x1y1 as f32),
    ) as f64
}

pub(self) fn lerp_3d(
    delta_x: f64,
    delta_y: f64,
    delta_z: f64,
    x0y0z0: f64,
    x1y0z0: f64,
    x0y1z0: f64,
    x1y1z0: f64,
    x0y0z1: f64,
    x1y0z1: f64,
    x0y1z1: f64,
    x1y1z1: f64,
) -> f64 {
    lerp(
        delta_z as f32,
        lerp_2d(delta_x, delta_y, x0y0z0, x1y0z0, x0y1z0, x1y1z0) as f32,
        lerp_2d(delta_x, delta_y, x0y0z1, x1y0z1, x0y1z1, x1y1z1) as f32,
    ) as f64
}

pub(self) fn smooth_step_derivative(val: f64) -> f64 {
    30.0 * val * val * (val - 1.0) * (val - 1.0)
}

pub(self) fn wrap(val: f64) -> f64 {
    const WRAP_CONST: f64 = 3.3554432E7;
    val - lfloor(val / WRAP_CONST + 0.5) as f64 * WRAP_CONST
}

pub(self) fn lfloor(val: f64) -> i64 {
    let l = val as i64;
    if val < l as f64 {
        l - 1
    } else {
        l
    }
}

pub struct NoiseSamplingSettings {
    xz_scale: f64,
    y_scale: f64,
    xz_factor: f64,
    y_factor: f64,
}
