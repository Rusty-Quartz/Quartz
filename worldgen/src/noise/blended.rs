use std::ops::Range;

use quartz_util::math::LerpExt;

use crate::{
    density_function::DensityFunctionContext,
    noise::{perlin::PerlinNoise, wrap},
    random::{xoroshiro::XoroshiroRandom, RandomSource},
};

const fn arr_from_range<const SIZE: usize>(r: Range<i32>, neg: bool) -> [i32; SIZE] {
    let mut x = [0; SIZE];
    let mut i = 0;
    let mut val = r.start;
    let end = r.end;

    loop {
        x[i] = val;
        i += 1;
        if neg {
            val -= 1;
        } else {
            val += 1;
        }

        if val == end {
            break;
        }
    }

    x
}

#[derive(Clone)]
pub struct BlendedNoise {
    min_limit_noise: PerlinNoise,
    max_limit_noise: PerlinNoise,
    main_noise: PerlinNoise,
    xz_factor: f64,
    y_factor: f64,
    xz_scale: f64,
    y_scale: f64,
    smear_scale_multiplier: f64,
    xz_multiplier: f64,
    y_multiplier: f64,
    max_value: f64,
}

impl BlendedNoise {
    pub fn new_from_random(
        rand_source: &mut impl RandomSource,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        Self::new(
            PerlinNoise::create_legacy_for_blended_noise(
                rand_source,
                &arr_from_range::<15>(-15 .. 0, false),
            ),
            PerlinNoise::create_legacy_for_blended_noise(
                rand_source,
                &arr_from_range::<15>(-15 .. 0, false),
            ),
            PerlinNoise::create_legacy_for_blended_noise(
                rand_source,
                &arr_from_range::<7>(-7 .. 0, false),
            ),
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
        )
    }

    pub fn fork_with_random(&self, rand_source: &mut impl RandomSource) -> Self {
        BlendedNoise::new_from_random(
            rand_source,
            self.xz_scale,
            self.y_scale,
            self.xz_factor,
            self.y_factor,
            self.smear_scale_multiplier,
        )
    }

    pub fn new_unseeded(
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> Self {
        Self::new_from_random(
            &mut XoroshiroRandom::new(0),
            xz_scale,
            y_scale,
            xz_factor,
            y_factor,
            smear_scale_multiplier,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        min_limit_noise: PerlinNoise,
        max_limit_noise: PerlinNoise,
        main_noise: PerlinNoise,
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    ) -> BlendedNoise {
        BlendedNoise {
            max_value: min_limit_noise.max_broken_value(y_scale),
            min_limit_noise,
            max_limit_noise,
            main_noise,
            xz_scale,
            y_scale,
            smear_scale_multiplier,
            xz_multiplier: 684.412 * xz_scale,
            y_multiplier: 684.412 * y_scale,
            xz_factor,
            y_factor,
        }
    }

    pub fn calculate<C: DensityFunctionContext>(&self, ctx: &C) -> f64 {
        let pos = ctx.get_pos();
        let scaled_x = pos.x as f64 * self.xz_multiplier / self.xz_factor;
        let scaled_y = pos.y as f64 * self.y_multiplier / self.y_factor;
        let scaled_z = pos.z as f64 * self.xz_multiplier / self.xz_factor;
        let y_smeared_scale_factor = self.y_multiplier * self.smear_scale_multiplier;
        let k = y_smeared_scale_factor / self.y_factor;
        let mut total_noise = 0.0;
        let mut scale = 1.0;


        for octave in 0 .. 8 {
            let noise = self.main_noise.get_octave_noise(octave);
            if let Some(noise) = noise {
                total_noise += noise.scaled_noise(
                    wrap(scaled_x * scale),
                    wrap(scaled_y * scale),
                    wrap(scaled_z * scale),
                    k * scale,
                    scaled_y * scale,
                ) / scale
            }

            scale /= 2.0;
        }

        let adjusted_noise = total_noise / 20.0 + 0.5;
        let mut scale = 1.0;
        let less = adjusted_noise < 1.0;
        let greater = adjusted_noise > 0.0;
        let mut min_noise = 0.0;
        let mut max_noise = 0.0;

        for octave in 0 .. 16 {
            let rescaled_x = wrap(scaled_x * scale);
            let rescaled_y = wrap(scaled_y * scale);
            let rescaled_z = wrap(scaled_z * scale);
            let y_scale = y_smeared_scale_factor * scale;

            if less {
                let noise = self.min_limit_noise.get_octave_noise(octave);
                if let Some(noise) = noise {
                    min_noise += noise.scaled_noise(
                        rescaled_x,
                        rescaled_y,
                        rescaled_z,
                        y_scale,
                        scaled_y * y_scale,
                    ) / scale;
                }
            }

            if greater {
                let noise = self.max_limit_noise.get_octave_noise(octave);
                if let Some(noise) = noise {
                    max_noise += noise.scaled_noise(
                        rescaled_x,
                        rescaled_y,
                        rescaled_z,
                        y_scale,
                        scaled_y * y_scale,
                    ) / scale;
                }
            }

            scale /= 2.0;
        }

        LerpExt::clamped_lerp(adjusted_noise, min_noise / 512.0, max_noise / 512.0) / 128.0
    }

    pub fn max_value(&self) -> f64 {
        self.max_value
    }

    pub fn min_value(&self) -> f64 {
        -self.max_value
    }
}
