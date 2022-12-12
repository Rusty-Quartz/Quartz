use quartz_util::math::{div_floor, LerpExt};

use crate::{
    density_function::DensityFunctionContext,
    noise::{perlin::PerlinNoise, wrap, NoiseSamplingSettings},
};


#[derive(Clone)]
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

    pub fn calculate<C: DensityFunctionContext>(&mut self, ctx: &C) -> f64 {
        let pos = ctx.get_pos();
        let x = div_floor(pos.x, self.cell_width);
        let y = div_floor(pos.y as i32, self.cell_height);
        let z = div_floor(pos.z, self.cell_width);

        let mut d = 0.0;
        let mut e = 0.0;
        let mut f = 0.0;
        let mut bl = true;
        let mut scale = 1.0;

        for i in 0 .. 8 {
            let noise = self.main_noise.get_octave_noise(i);
            match noise {
                Some(noise) =>
                    f += noise.scaled_noise(
                        wrap(x as f64 * self.xz_main_scale * scale),
                        wrap(y as f64 * self.y_main_scale * scale),
                        wrap(z as f64 * self.xz_main_scale * scale),
                        self.y_main_scale * scale,
                        y as f64 * self.y_main_scale * scale,
                    ) / scale,
                None => {}
            }

            scale /= 2.0;
        }

        let h = (f / 10.0 + 1.0) / 2.0;
        let bl2 = h >= 1.0;
        let bl3 = h <= 0.0;
        scale = 1.0;

        for i in 0 .. 16 {
            let scaled_x = wrap(x as f64 * self.xz_scale * scale);
            let scaled_y = wrap(y as f64 * self.y_scale * scale);
            let scaled_z = wrap(z as f64 * self.xz_scale * scale);
            let y_scale = self.y_scale * scale;

            if !bl2 {
                let noise = self.min_limit_noise.get_octave_noise(i);
                if let Some(noise) = noise {
                    d += noise.scaled_noise(
                        scaled_x,
                        scaled_y,
                        scaled_z,
                        y_scale,
                        y as f64 * y_scale,
                    ) / scale;
                }
            }

            if !bl3 {
                let noise = self.max_limit_noise.get_octave_noise(i);
                if let Some(noise) = noise {
                    e += noise.scaled_noise(
                        scaled_x,
                        scaled_y,
                        scaled_z,
                        y_scale,
                        y as f64 * y_scale,
                    ) / scale;
                }
            }

            scale /= 2.0;
        }

        LerpExt::clamped_lerp(h, d / 512.0, e / 512.0) / 128.0
    }
}
