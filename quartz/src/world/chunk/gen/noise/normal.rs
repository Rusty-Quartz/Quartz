use crate::world::chunk::gen::{noise::perlin::PerlinNoise, random::RandomSource};

const INPUT_FACTOR: f64 = 1.0181268882175227;
const TARGET_DEVIATION: f64 = 0.3333333333333333;

#[derive(Clone)]
pub struct NormalNoise {
    value_factor: f64,
    first_noise: PerlinNoise,
    second_noise: PerlinNoise,
    max_value: f64,
    parameters: NoiseParamteres,
}

impl NormalNoise {
    pub fn create_legacy_nether_biome(
        rand_source: &mut impl RandomSource,
        params: NoiseParamteres,
    ) -> NormalNoise {
        NormalNoise::new(rand_source, params, false)
    }

    pub fn create_noise(
        rand_source: &mut impl RandomSource,
        params: NoiseParamteres,
    ) -> NormalNoise {
        NormalNoise::new(rand_source, params, true)
    }

    fn new(
        rand_source: &mut impl RandomSource,
        params: NoiseParamteres,
        use_legacy_nether_biome: bool,
    ) -> NormalNoise {
        let first_octave = params.first_octave;
        let amplitudes = params.amplitudes.clone();

        let (first_noise, second_noise) = if use_legacy_nether_biome {
            (
                PerlinNoise::new(rand_source, (first_octave, amplitudes.clone()), true),
                PerlinNoise::new(rand_source, (first_octave, amplitudes.clone()), true),
            )
        } else {
            (
                PerlinNoise::create_legacy_for_legacy_nether_biome(
                    rand_source,
                    first_octave,
                    amplitudes.clone(),
                ),
                PerlinNoise::create_legacy_for_legacy_nether_biome(
                    rand_source,
                    first_octave,
                    amplitudes.clone(),
                ),
            )
        };

        let mut min = i32::MAX;
        let mut max = i32::MIN;

        for (i, amplitude) in amplitudes.iter().copied().enumerate() {
            if amplitude != 0.0 {
                min = i32::min(min, i as i32);
                max = i32::max(max, i as i32);
            }
        }

        let value_factor = (TARGET_DEVIATION / 2.0) / NormalNoise::expected_deviation(max - min);

        NormalNoise {
            value_factor,
            max_value: (first_noise.max_value + second_noise.max_value) * value_factor,
            first_noise,
            second_noise,
            parameters: params,
        }
    }

    fn expected_deviation(octaves: i32) -> f64 {
        0.1 * (1.0 + 1.0 / (octaves + 1) as f64)
    }

    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let ox = x * 1.0181268882175227;
        let oy = y * 1.0181268882175227;
        let oz = z * 1.0181268882175227;

        (self.first_noise.get_value_simple(x, y, z)
            + self.second_noise.get_value_simple(ox, oy, oz))
            * self.value_factor
    }
}

#[derive(Clone)]
pub struct NoiseParamteres {
    first_octave: i32,
    amplitudes: Vec<f64>,
}
