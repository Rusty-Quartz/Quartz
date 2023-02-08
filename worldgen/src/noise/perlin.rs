use quartz_util::math::{dot, lerp_2d, lerp_3d, smooth_step, smooth_step_derivative, wrap};

use crate::{
    noise::simplex::SimplexOctave,
    random::{PositionalRandomBuilder, RandomSource},
};

/// Generates multiple octaves of perlin noise.
#[derive(Clone)]
pub struct PerlinNoise {
    octaves: Vec<Option<PerlinOctave>>,
    first_octave: i32,
    amplitudes: Vec<f64>,
    lowest_frequency_value_factor: f64,
    lowest_frequency_input_factor: f64,
    pub(super) max_value: f64,
}

impl PerlinNoise {
    /// # Panics
    /// Panics if octaves is empty or if the -first + last is less than one
    pub fn create_legacy_for_blended_noise(
        rand_source: &mut impl RandomSource,
        octaves: &[i32],
    ) -> PerlinNoise {
        PerlinNoise::new(rand_source, PerlinNoise::make_amplitudes(octaves), false)
    }

    pub fn create_legacy_for_legacy_nether_biome(
        rand_source: &mut impl RandomSource,
        first_octave: i32,
        octaves: Vec<f64>,
    ) -> PerlinNoise {
        PerlinNoise::new(rand_source, (first_octave, octaves), false)
    }

    /// # Panics
    /// Panics if octaves is empty or if the -first + last is less than one
    pub fn from_octaves(rand_source: &mut impl RandomSource, octaves: &[i32]) -> PerlinNoise {
        PerlinNoise::new(rand_source, PerlinNoise::make_amplitudes(octaves), true)
    }

    pub fn from_amplitudes(
        rand_source: &mut impl RandomSource,
        first_octave: i32,
        amplitudes: Vec<f64>,
    ) -> PerlinNoise {
        PerlinNoise::new(rand_source, (first_octave, amplitudes), true)
    }

    pub fn new(
        rand_source: &mut impl RandomSource,
        amplitudes: (i32, Vec<f64>),
        bl: bool,
    ) -> PerlinNoise {
        let first_octave = amplitudes.0;
        let amplitudes = amplitudes.1;
        let octave_count = amplitudes.len() as i32;
        let j = -first_octave;
        let mut noise_levels = vec![None; octave_count as usize];

        if bl {
            let pos_random = rand_source.fork_positional();

            for k in 0 .. octave_count as usize {
                if amplitudes[k] != 0.0 {
                    let l = first_octave + k as i32;
                    noise_levels[k] = Some(PerlinOctave::new(
                        &mut pos_random.fork_from_hashed_string(format!("octave_{l}")),
                    ));
                }
            }
        } else {
            let octave = Some(PerlinOctave::new(rand_source));
            if (0 .. 1).contains(&j) {
                let d = amplitudes[j as usize];
                if d != 0.0 {
                    noise_levels[j as usize] = octave;
                }
            }

            for k in j as usize - 1 .. 0 {
                if (k as i32) < octave_count {
                    let e = amplitudes[k];
                    if e != 0.0 {
                        noise_levels[k] = Some(PerlinOctave::new(rand_source));
                    } else {
                        PerlinNoise::skip_octave(rand_source)
                    }
                } else {
                    PerlinNoise::skip_octave(rand_source)
                }
            }
        }

        let mut noise = PerlinNoise {
            octaves: noise_levels,
            first_octave,
            amplitudes,
            lowest_frequency_input_factor: f64::powi(2.0, -j),
            lowest_frequency_value_factor: f64::powi(2.0, octave_count - 1)
                / (f64::powi(2.0, octave_count) - 1.0),
            max_value: 0.0,
        };

        let max_val = noise.edge_value(2.0);

        noise.max_value = max_val;
        noise
    }

    pub fn get_value_simple(&self, x: f64, y: f64, z: f64) -> f64 {
        self.get_value(x, y, z, 0.0, 0.0, false)
    }

    pub fn get_value(
        &self,
        x: f64,
        y: f64,
        z: f64,
        y_scale: f64,
        y_max: f64,
        use_fixed_y: bool,
    ) -> f64 {
        let mut val = 0.0;
        let mut input_factor = self.lowest_frequency_input_factor;
        let mut val_factor = self.lowest_frequency_value_factor;

        for (amplitude, octave) in self.amplitudes.iter().zip(self.octaves.iter()) {
            if let Some(octave) = octave {
                let noise_val = octave.scaled_noise(
                    wrap(x * input_factor),
                    if use_fixed_y {
                        -octave.y_offset
                    } else {
                        wrap(y * input_factor)
                    },
                    wrap(z * input_factor),
                    y_scale * input_factor,
                    y_max * input_factor,
                );
                val += amplitude * noise_val * val_factor;
            }

            input_factor *= 2.0;
            val_factor /= 2.0;
        }

        val
    }

    pub fn max_broken_value(&self, d: f64) -> f64 {
        self.edge_value(d + 2.0)
    }

    fn edge_value(&self, d: f64) -> f64 {
        let mut val = 0.0;
        let mut val_factor = self.lowest_frequency_value_factor;

        for (amplitude, octave) in self.amplitudes.iter().zip(self.octaves.iter()) {
            if octave.is_some() {
                val += amplitude * d * val_factor;
            }

            val_factor /= 2.0;
        }

        val
    }

    fn make_amplitudes(octaves: &[i32]) -> (i32, Vec<f64>) {
        if octaves.is_empty() {
            panic!("Perlin noise has to be given octaves");
        }
        let first_octave = -octaves[0];
        let last_octave = octaves[octaves.len()];
        let octave_total = first_octave + last_octave + 1;
        if octave_total < 1 {
            panic!("Perlin noise has to have more than one octave");
        }

        let mut double_list = Vec::with_capacity(octave_total as usize);

        for octave in octaves {
            // Shouldn't panic because we should have guarenteed double list is of the right size
            double_list[(octave + first_octave) as usize] = 1.0;
        }


        (-first_octave, double_list)
    }

    fn skip_octave(rand_source: &mut impl RandomSource) {
        rand_source.consume(262);
    }

    pub fn get_octave_noise(&self, octave: usize) -> Option<&PerlinOctave> {
        let octaves_count = self.octaves.len();
        self.octaves
            .get(octaves_count - 1 - octave)
            .and_then(Option::as_ref)
    }

    pub(super) fn first_octave(&self) -> i32 {
        self.first_octave
    }
}

#[derive(Clone)]
pub struct PerlinOctave {
    permutations: [u8; 256],
    x_offset: f64,
    y_offset: f64,
    z_offset: f64,
}

impl PerlinOctave {
    pub const SHIFT_UP_EPSILON: f64 = 1.0e-7;

    pub fn new(random_source: &mut impl RandomSource) -> PerlinOctave {
        let xo = random_source.next_double() * 256.0;
        let yo = random_source.next_double() * 256.0;
        let zo = random_source.next_double() * 256.0;
        let mut permutations: [u8; 256] = [0; 256];

        for (entry, val) in permutations.iter_mut().zip(0 ..= 255) {
            *entry = val;
        }

        for i in 0 .. 256 {
            let rand_int = random_source.next_int_bounded(256 - i as u32) as usize;
            permutations.swap(i, i + rand_int);
        }

        Self {
            x_offset: xo,
            y_offset: yo,
            z_offset: zo,
            permutations,
        }
    }

    pub fn noise(&mut self, x: f64, y: f64, z: f64) -> f64 {
        self.scaled_noise(x, y, z, 0.0, 0.0)
    }

    // apparently this is deprecated?
    // I think this just means that people making mc mods shouldn't use it
    // not that its going to get removed or anything
    pub fn scaled_noise(&self, x: f64, y: f64, z: f64, y_scale: f64, y_max: f64) -> f64 {
        let offset_x = x + self.x_offset;
        let offset_y = y + self.y_offset;
        let offset_z = z + self.z_offset;

        let grid_x = x as i32;
        let grid_y = y as i32;
        let grid_z = z as i32;

        let delta_x = offset_x - grid_x as f64;
        let delta_y = offset_y - grid_y as f64;
        let delta_z = offset_z - grid_z as f64;

        // don't know what to call this lol so here we are
        let mut weird_factor = 0.0;
        if y_scale != 0.0 {
            let mut max = delta_y;
            if y_max >= 0.0 && y_max < delta_y {
                max = y_max
            }

            weird_factor = (max / y_scale + Self::SHIFT_UP_EPSILON).floor() * y_scale
        }

        self.sample_and_lerp(
            grid_x,
            grid_y,
            grid_z,
            delta_x,
            delta_y - weird_factor,
            delta_z,
            delta_y,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn sample_and_lerp(
        &self,
        grid_x: i32,
        grid_y: i32,
        grid_z: i32,
        delta_x: f64,
        weird_delta_y: f64,
        delta_z: f64,
        delta_y: f64,
    ) -> f64 {
        let left = self.p(grid_x);
        let right = self.p(grid_x + 1);
        let top_left = self.p(left + grid_y);
        let bottom_left = self.p(left + grid_y + 1);
        let top_right = self.p(right + grid_y);
        let bottom_right = self.p(right + grid_y + 1);

        let x0y0z0 =
            PerlinOctave::grad_dot(self.p(top_left + grid_z), delta_x, weird_delta_y, delta_z);
        let x1y0z0 = PerlinOctave::grad_dot(
            self.p(top_right + grid_z),
            delta_x - 1.0,
            weird_delta_y,
            delta_z,
        );
        let x0y1z0 = PerlinOctave::grad_dot(
            self.p(bottom_left + grid_z),
            delta_x,
            weird_delta_y - 1.0,
            delta_z,
        );
        let x1y1z0 = PerlinOctave::grad_dot(
            self.p(bottom_right + grid_z),
            delta_x - 1.0,
            weird_delta_y,
            delta_z,
        );
        let x0y0z1 = PerlinOctave::grad_dot(
            self.p(top_left + grid_z + 1),
            delta_x,
            weird_delta_y,
            delta_z - 1.0,
        );
        let x1y0z1 = PerlinOctave::grad_dot(
            self.p(top_right + grid_z + 1),
            delta_x - 1.0,
            weird_delta_y,
            delta_z - 1.0,
        );
        let x0y1z1 = PerlinOctave::grad_dot(
            self.p(bottom_left + grid_z + 1),
            delta_x,
            weird_delta_y - 1.0,
            delta_z - 1.0,
        );
        let x1y1z1 = PerlinOctave::grad_dot(
            self.p(top_left + grid_z + 1),
            delta_x - 1.0,
            weird_delta_y - 1.0,
            delta_z - 1.0,
        );

        let smoothed_delta_x = smooth_step(delta_x);
        let smoothed_delta_y = smooth_step(delta_y);
        let smoothed_delta_z = smooth_step(delta_z);

        lerp_3d(
            smoothed_delta_x,
            smoothed_delta_y,
            smoothed_delta_z,
            x0y0z0,
            x1y0z0,
            x0y1z0,
            x1y1z0,
            x0y0z1,
            x1y0z1,
            x0y1z1,
            x1y1z1,
        )
    }

    fn p(&self, index: i32) -> i32 {
        // mojang never cast their permutation value
        // to an int, so while it doesn't seem like these &0xFFs
        // do anything I'm keeping them for now
        self.permutations[index as usize & 0xFF] as i32 & 0xFF
    }

    fn grad_dot(grad_index: i32, x_factor: f64, y_factor: f64, z_factor: f64) -> f64 {
        dot(
            SimplexOctave::GRADIENT[grad_index as usize & 15],
            x_factor,
            y_factor,
            z_factor,
        )
    }

    pub fn noise_with_derivative(&self, x: f64, y: f64, z: f64, values: &mut [f64]) -> f64 {
        let offset_x = x + self.x_offset;
        let offset_y = y + self.y_offset;
        let offset_z = z + self.z_offset;
        let grid_x = offset_x as i32;
        let grid_y = offset_y as i32;
        let grid_z = offset_z as i32;
        let delta_x = offset_x - grid_x as f64;
        let delta_y = offset_y - grid_y as f64;
        let delta_z = offset_z - grid_z as f64;
        self.sample_with_derivative(grid_x, grid_y, grid_z, delta_x, delta_y, delta_z, values)
    }

    #[allow(clippy::too_many_arguments)]
    fn sample_with_derivative(
        &self,
        grid_x: i32,
        grid_y: i32,
        grid_z: i32,
        delta_x: f64,
        delta_y: f64,
        delta_z: f64,
        values: &mut [f64],
    ) -> f64 {
        let x0 = self.p(grid_x);
        let x1 = self.p(grid_x + 1);
        let x0y0 = self.p(x0 + grid_y);
        let x0y1 = self.p(x0 + grid_y + 1);
        let x1y0 = self.p(x1 + grid_y);
        let x1y1 = self.p(x1 + grid_y + 1);
        let x0y0z0 = self.p(x0y0 + grid_z);
        let x0y1z0 = self.p(x0y1 + grid_z);
        let x1y0z0 = self.p(x1y0 + grid_z);
        let x1y1z0 = self.p(x1y1 + grid_z);
        let x0y0z1 = self.p(x0y0 + grid_z + 1);
        let x0y1z1 = self.p(x0y1 + grid_z + 1);
        let x1y0z1 = self.p(x1y0 + grid_z + 1);
        let x1y1z1 = self.p(x1y1 + grid_z + 1);

        let x0y0z0_grad = SimplexOctave::GRADIENT[x0y0z0 as usize & 15];
        let x0y1z0_grad = SimplexOctave::GRADIENT[x0y1z0 as usize & 15];
        let x1y0z0_grad = SimplexOctave::GRADIENT[x1y0z0 as usize & 15];
        let x1y1z0_grad = SimplexOctave::GRADIENT[x1y1z0 as usize & 15];
        let x0y0z1_grad = SimplexOctave::GRADIENT[x0y0z1 as usize & 15];
        let x0y1z1_grad = SimplexOctave::GRADIENT[x0y1z1 as usize & 15];
        let x1y0z1_grad = SimplexOctave::GRADIENT[x1y0z1 as usize & 15];
        let x1y1z1_grad = SimplexOctave::GRADIENT[x1y1z1 as usize & 15];

        let x0y0z0_dot = dot(x0y0z0_grad, delta_x, delta_y, delta_z);
        let x0y1z0_dot = dot(x0y1z0_grad, delta_x - 1.0, delta_y, delta_z);
        let x1y0z0_dot = dot(x1y0z0_grad, delta_x, delta_y - 1.0, delta_z);
        let x1y1z0_dot = dot(x1y1z0_grad, delta_x - 1.0, delta_y - 1.0, delta_z);
        let x0y0z1_dot = dot(x0y0z1_grad, delta_x, delta_y, delta_z - 1.0);
        let x0y1z1_dot = dot(x0y1z1_grad, delta_x - 1.0, delta_y, delta_z - 1.0);
        let x1y0z1_dot = dot(x1y0z1_grad, delta_x, delta_y - 1.0, delta_z - 1.0);
        let x1y1z1_dot = dot(x1y1z1_grad, delta_x - 1.0, delta_y - 1.0, delta_z - 1.0);

        let smoothed_delta_x = smooth_step(delta_x);
        let smoothed_delta_y = smooth_step(delta_y);
        let smoothed_delta_z = smooth_step(delta_z);

        let ac = lerp_3d(
            smoothed_delta_x,
            smoothed_delta_y,
            smoothed_delta_z,
            x0y0z0_grad[0] as f64,
            x1y0z0_grad[0] as f64,
            x0y1z0_grad[0] as f64,
            x1y1z0_grad[0] as f64,
            x0y0z1_grad[0] as f64,
            x1y0z1_grad[0] as f64,
            x0y1z1_grad[0] as f64,
            x1y1z1_grad[0] as f64,
        );
        let ad = lerp_3d(
            smoothed_delta_x,
            smoothed_delta_y,
            smoothed_delta_z,
            x0y0z0_grad[1] as f64,
            x1y0z0_grad[1] as f64,
            x0y1z0_grad[1] as f64,
            x1y1z0_grad[1] as f64,
            x0y0z1_grad[1] as f64,
            x1y0z1_grad[1] as f64,
            x0y1z1_grad[1] as f64,
            x1y1z1_grad[1] as f64,
        );
        let ae = lerp_3d(
            smoothed_delta_x,
            smoothed_delta_y,
            smoothed_delta_z,
            x0y0z0_grad[2] as f64,
            x1y0z0_grad[2] as f64,
            x0y1z0_grad[2] as f64,
            x1y1z0_grad[2] as f64,
            x0y0z1_grad[2] as f64,
            x1y0z1_grad[2] as f64,
            x0y1z1_grad[2] as f64,
            x1y1z1_grad[2] as f64,
        );

        let af = lerp_2d(
            smoothed_delta_y,
            smoothed_delta_z,
            x1y0z0_dot - x0y0z0_dot,
            x1y1z0_dot - x0y1z0_dot,
            x1y0z1_dot - x0y0z1_dot,
            x1y1z1_dot - x0y1z1_dot,
        );
        let ag = lerp_2d(
            smoothed_delta_y,
            smoothed_delta_z,
            x0y1z0_dot - x0y0z0_dot,
            x0y1z1_dot - x0y0z1_dot,
            x1y1z0_dot - x1y0z0_dot,
            x1y1z1_dot - x1y0z1_dot,
        );
        let ah = lerp_2d(
            smoothed_delta_y,
            smoothed_delta_z,
            x0y0z1_dot - x0y0z0_dot,
            x1y0z1_dot - x1y0z0_dot,
            x0y1z1_dot - x0y1z0_dot,
            x1y1z1_dot - x1y1z0_dot,
        );

        let smoothed_deriv_x = smooth_step_derivative(delta_x);
        let smoothed_deriv_y = smooth_step_derivative(delta_y);
        let smoothed_deriv_z = smooth_step_derivative(delta_z);

        let al = ac + smoothed_deriv_x * af;
        let am = ad + smoothed_deriv_y * ag;
        let an = ae + smoothed_deriv_z * ah;

        values[0] += al;
        values[1] += am;
        values[2] += an;

        lerp_3d(
            smoothed_delta_x,
            smoothed_delta_y,
            smoothed_delta_z,
            x0y0z0_dot,
            x1y0z0_dot,
            x0y1z0_dot,
            x1y1z0_dot,
            x0y0z1_dot,
            x1y0z1_dot,
            x0y1z1_dot,
            x1y1z1_dot,
        )
    }
}
