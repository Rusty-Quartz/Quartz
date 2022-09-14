use quartz_util::math::LerpExt;

pub mod blended;
pub mod normal;
pub mod perlin;
pub mod simplex;

// Utility math functions used in noise
pub(self) fn lerp_2d(
    delta_x: f64,
    delta_y: f64,
    x0y0: f64,
    x1y0: f64,
    x0y1: f64,
    x1y1: f64,
) -> f64 {
    LerpExt::lerp(
        delta_y as f32,
        LerpExt::lerp(delta_x as f32, x0y0 as f32, x1y0 as f32),
        LerpExt::lerp(delta_x as f32, x0y1 as f32, x1y1 as f32),
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
    LerpExt::lerp(
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
