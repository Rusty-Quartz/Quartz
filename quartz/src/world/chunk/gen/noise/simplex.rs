use crate::world::chunk::gen::{
    noise::dot,
    random::{legacy_random::LegacyRandom, worldgen::WorldgenRandom, RandomSource},
};


pub struct SimplexNoise {
    octaves: Vec<Option<SimplexOctave>>,
    highest_freq_val_factor: f64,
    highest_freq_input_factor: f64,
}

impl SimplexNoise {
    pub fn new(rand_source: &mut impl RandomSource, octaves: Vec<i32>) -> SimplexNoise {
        if octaves.is_empty() {
            panic!("Simplex noise needs octaves")
        }

        let i = -octaves[0];
        let last_octave = octaves[octaves.len() - 1];
        let octave_count = i + last_octave + 1;

        if octave_count < 1 {
            panic!("Simplex noise needs more than one octave")
        }

        let octave = SimplexOctave::new(rand_source);
        let mut l = last_octave;
        let mut noise_levels = vec![None; octave_count as usize];

        if (0 .. octave_count).contains(&last_octave) && octaves.contains(&0) {
            noise_levels[last_octave as usize] = Some(octave.clone());
        }

        // honestly the suggested code for this lint is less clear than this for loop so fuck it
        #[warn(clippy::needless_range_loop)]
        for m in last_octave + 1 .. octave_count {
            if m >= 0 && octaves.contains(&(l - m as i32)) {
                noise_levels[m as usize] = Some(SimplexOctave::new(rand_source));
            } else {
                rand_source.consume(262);
            }
        }

        if last_octave > 0 {
            let n = octave.sample_3d(octave.xo, octave.yo, octave.zo) * 9.223372E18;
            let mut world_rand_source = WorldgenRandom::new(LegacyRandom::new(n as i64));

            for i in l - 1 .. 0 {
                if i < octave_count && octaves.contains(&(l - i)) {
                    noise_levels[i as usize] = Some(SimplexOctave::new(&mut world_rand_source))
                } else {
                    world_rand_source.consume(262);
                }
            }
        }

        SimplexNoise {
            octaves: noise_levels,
            highest_freq_val_factor: f64::powi(2.0, last_octave),
            highest_freq_input_factor: 1.0 / (f64::powi(2.0, octave_count) - 1.0),
        }
    }

    pub fn get_value(&mut self, x: f64, y: f64, use_noise_offsets: bool) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.highest_freq_input_factor;
        let mut value_factor = self.highest_freq_val_factor;

        for octave in &self.octaves {
            if let Some(octave) = octave {
                let (x_offset, y_offset) = if use_noise_offsets {
                    (octave.xo, octave.yo)
                } else {
                    (0.0, 0.0)
                };

                value += octave.sample_2d(x * input_factor + x_offset, y * input_factor + y_offset)
                    * value_factor;
            }

            input_factor /= 2.0;
            value_factor *= 2.0;
        }

        value
    }
}

#[derive(Clone)]
pub struct SimplexOctave {
    // this is actually a [u32; 512] in the vanilla server, but only 256 are used
    // if this changes post 1.18.2 we'll update it
    permutation_table: [u32; 256],
    // these are declared but not used in vanilla?
    xo: f64,
    yo: f64,
    zo: f64,
}

impl SimplexOctave {
    // These constants *might* be commonly called stretch and squish
    // I don't know, and honestly rn I don't care
    const F2: f64 = 0.5 * (SimplexOctave::SQRT_3 - 1.0);
    const G2: f64 = (3.0 - SimplexOctave::SQRT_3) / 6.0;
    pub const GRADIENT: [[i32; 3]; 16] = [
        [1, 1, 0],
        [-1, 1, 0],
        [1, -1, 0],
        [-1, -1, 0],
        [1, 0, 1],
        [-1, 0, 1],
        [1, 0, -1],
        [-1, 0, -1],
        [0, 1, 1],
        [0, -1, 1],
        [0, 1, -1],
        [0, -1, -1],
        [1, 1, 0],
        [0, -1, 1],
        [-1, 1, 0],
        [0, -1, -1],
    ];
    // calculated by just running java's sqrt function
    // rust please add constant float operations
    const SQRT_3: f64 = 1.7320508075688772;

    fn get_perm(&self, index: i32) -> u32 {
        self.permutation_table[index as usize & 0xFF]
    }

    #[allow(clippy::unnecessary_operation)]
    pub fn new<R: RandomSource>(rand_source: &mut R) -> Self {
        // if xo, yo, and zo end up being used these are their initializers
        // leaving them in because they technically modify the random source so randomness would be off without them
        let xo = rand_source.next_double() * 256.0;
        let yo = rand_source.next_double() * 256.0;
        let zo = rand_source.next_double() * 256.0;

        // initialize the permutation table
        let mut perm_table = [0; 256];
        for (ele, val) in perm_table.iter_mut().zip(0 .. 256) {
            *ele = val;
        }

        // randomly shuffle the permutation table
        for i in 0 .. 256_usize {
            let rand_index = rand_source.next_int_bounded(256 - i as u32) as usize;
            perm_table.swap(i, i + rand_index);
        }

        SimplexOctave {
            permutation_table: perm_table,
            xo,
            yo,
            zo,
        }
    }

    fn get_corner_noise_3d(gradient_index: usize, x: f64, y: f64, z: f64, offset: f64) -> f64 {
        let d = offset - x * x - y * y - z * z;
        if d < 0.0 {
            0.0
        } else {
            d * d * d * d * dot(SimplexOctave::GRADIENT[gradient_index], x, y, z)
        }
    }

    // I have no idea how any of this works
    // TODO: come back and label these vars properly
    fn sample_2d(&self, x: f64, y: f64) -> f64 {
        let d: f64 = (x + y) * SimplexOctave::F2;
        let i = (x + d).floor();
        let j = (y + d).floor();
        let e = (i + j) * SimplexOctave::G2;
        let f = i - e;
        let g = j - e;
        let h = x - f;
        let k = y - g;
        let (l, m) = if h > k { (1, 0) } else { (0, 1) };

        let n = h - l as f64 + SimplexOctave::G2;
        let o = k - m as f64 + SimplexOctave::G2;
        let p = h - 1.0 + 2.0 * SimplexOctave::G2;
        let q = k - 1.0 + 2.0 * SimplexOctave::G2;
        let r = i as i32 & 0xFF;
        let s = j as i32 & 0xFF;
        let t = self.get_perm(r + self.get_perm(s) as i32) as i32 % 12;
        let u = self.get_perm(r + l + self.get_perm(s + m) as i32) as i32 % 12;
        let v = self.get_perm(r + 1 + self.get_perm(s + 1) as i32) as i32 % 12;
        let w = SimplexOctave::get_corner_noise_3d(t as usize, h, k, 0.0, 0.5);
        let z = SimplexOctave::get_corner_noise_3d(u as usize, n, o, 0.0, 0.5);
        let aa = SimplexOctave::get_corner_noise_3d(v as usize, p, q, 0.0, 0.5);
        70.0 * (w + z + aa)
    }

    fn sample_3d(&self, x: f64, y: f64, z: f64) -> f64 {
        // we do this instead of x/3
        // because this is what mojang does
        // and it might have different rounding characteristics
        const THIRD: f64 = 0.3333333333333333;
        const SIXTH: f64 = 0.16666666666666666;

        let e = (x + y + z) * THIRD;
        let i = (x + e).floor();
        let j = (y + e).floor();
        let k = (z + e).floor();

        let g = (i + j + k) * SIXTH;
        let h = i - g;
        let l = j - g;
        let m = k - g;

        let n = x - h;
        let o = y - l;
        let p = z - m;

        let (q, r, s, t, u, v) = if n >= o {
            if o >= p {
                (1, 0, 0, 1, 1, 0)
            } else if n >= p {
                (1, 0, 0, 1, 0, 1)
            } else {
                (0, 0, 1, 1, 0, 1)
            }
        } else if o < p {
            (0, 0, 1, 0, 1, 1)
        } else if n < p {
            (0, 1, 0, 0, 1, 1)
        } else {
            (0, 1, 0, 1, 1, 0)
        };

        let w = n - q as f64 + SIXTH;
        let aa = o - r as f64 + SIXTH;
        let ab = p - s as f64 + SIXTH;
        let ac = n - t as f64 + THIRD;
        let ad = o - u as f64 + THIRD;
        let ae = p - v as f64 + THIRD;
        let af = n - 0.5;
        let ag = o - 0.5;
        let ah = p - 0.5;
        let ai = i as i32 & 0xFF;
        let aj = j as i32 & 0xFF;
        let ak = k as i32 & 0xFF;

        let al = self.get_perm(ai + self.get_perm(aj + self.get_perm(ak) as i32) as i32) % 12;
        let am = self
            .get_perm(ai + q + self.get_perm(aj + r + self.get_perm(ak + s) as i32) as i32)
            % 12;
        let an = self
            .get_perm(ai + t + self.get_perm(aj + u + self.get_perm(ak + v) as i32) as i32)
            % 12;
        let ao = self
            .get_perm(ai + 1 + self.get_perm(aj + 1 + self.get_perm(ak + 1) as i32) as i32)
            % 12;

        let ap = SimplexOctave::get_corner_noise_3d(al as usize, n, o, p, 0.6);
        let aq = SimplexOctave::get_corner_noise_3d(am as usize, w, aa, ab, 0.6);
        let ar = SimplexOctave::get_corner_noise_3d(an as usize, ac, ad, ae, 0.6);
        let at = SimplexOctave::get_corner_noise_3d(ao as usize, af, ag, ah, 0.6);
        32.0 * (ap + aq + ar + at)
    }
}
