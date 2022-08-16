use crate::world::chunk::gen::noise::{perlin::PerlinNoise, NoiseSamplingSettings};

pub struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    xz_scale: f64,
    y_scale: f64,
    xz_main_scale: f64,
    y_main_scale: f64,
    cell_width: i32,
    cell_height: i32,
    max_value: f64,
}

impl BlendedNoise {
    fn new(
        min_limit_noise: PerlinNoise,
        max_limit_noise: PerlinNoise,
        main_noise: PerlinNoise,
        noise_sampling_settings: NoiseSamplingSettings,
        cell_width: i32,
        cell_height: i32,
    ) -> BlendedNoise {
        let xz_scale = 684.412 * noise_sampling_settings.xz_scale;
        let y_scale = 684.412 * noise_sampling_settings.y_scale;

        BlendedNoise {
            max_value: min_limit_noise.max_broken_value(y_scale),
            min_limit_noise,
            max_limit_noise,
            main_noise,
            xz_scale,
            y_scale,
            xz_main_scale: xz_scale / noise_sampling_settings.xz_factor,
            y_main_scale: y_scale / noise_sampling_settings.y_factor,
            cell_width,
            cell_height,
        }
    }
}
