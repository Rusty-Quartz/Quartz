//! PRNG helper functions and types
//!
//! Most functions are taken directly from vanilla minecraft code and adapted to rust

use crate::random::{
    marsaglia_polar::MarsagliaPolarGaussian,
    util::{get_pos_seed, hash_string_md5, i64_seed_to_u128_seed},
    PositionalRandomBuilder,
    RandomSource,
};

/// Used to construct new instances of [XoroshiroRandomSource]
pub struct XoroshiroPositonalRandomBuilder {
    seed_low: i64,
    seed_high: i64,
}

impl XoroshiroPositonalRandomBuilder {
    pub fn new(low: i64, high: i64) -> Self {
        XoroshiroPositonalRandomBuilder {
            seed_low: low,
            seed_high: high,
        }
    }
}

impl PositionalRandomBuilder for XoroshiroPositonalRandomBuilder {
    type Source = XoroshiroRandom;

    fn fork_at(&self, x: i32, y: i32, z: i32) -> XoroshiroRandom {
        let pos_seed = get_pos_seed(x, y, z);
        XoroshiroRandom::from_longs(pos_seed ^ self.seed_low, self.seed_high)
    }

    fn fork_from_hashed_string(&self, str: impl AsRef<str>) -> XoroshiroRandom {
        let (low, high) = hash_string_md5(str.as_ref());
        XoroshiroRandom {
            rng: XoroshiroPlusPlus::new(low ^ self.seed_low, high ^ self.seed_high),
        }
    }
}


pub struct XoroshiroRandom {
    rng: XoroshiroPlusPlus,
}

impl XoroshiroRandom {
    pub fn new(seed: i64) -> Self {
        let (low, high) = i64_seed_to_u128_seed(seed);
        XoroshiroRandom {
            rng: XoroshiroPlusPlus::new(low, high),
        }
    }

    pub fn from_longs(seed_low: i64, seed_high: i64) -> Self {
        XoroshiroRandom {
            rng: XoroshiroPlusPlus::new(seed_low, seed_high),
        }
    }

    fn next_bits(&mut self, bits: u8) -> u64 {
        self.next_long() as u64 >> (64 - bits)
    }
}

impl RandomSource for XoroshiroRandom {
    type Positional = XoroshiroPositonalRandomBuilder;

    fn fork(&mut self) -> XoroshiroRandom {
        let low = self.rng.next_long();
        let high = self.rng.next_long();
        Self::from_longs(low, high)
    }

    fn fork_positional(&mut self) -> XoroshiroPositonalRandomBuilder {
        let low = self.rng.next_long();
        let high = self.rng.next_long();
        XoroshiroPositonalRandomBuilder::new(low, high)
    }

    fn consume(&mut self, count: usize) {
        for _ in 0 .. count {
            self.rng.next_long();
        }
    }

    fn next_long(&mut self) -> i64 {
        self.rng.next_long()
    }

    fn next_int(&mut self) -> i32 {
        self.rng.next_long() as i32
    }

    fn next_int_bounded(&mut self, bound: u32) -> i32 {
        let mut l = self.next_int() as i64 & 0xFFFFFFFF;
        let mut m = l.wrapping_mul(bound as i64);
        let mut n = m & 4294967295;
        if n < bound as i64 {
            let j = (!bound + 1) % bound;
            while n < j as i64 {
                l = self.next_int() as i64;
                m = l.wrapping_mul(bound as i64);
                n = m & 4294967295;
            }
        }

        (m >> 32) as i32
    }

    fn next_bool(&mut self) -> bool {
        self.rng.next_long() & 1 != 0
    }

    fn next_float(&mut self) -> f32 {
        // hopefully this works?
        self.next_bits(24) as f32 * 5.9604645E-8
    }

    fn next_double(&mut self) -> f64 {
        self.next_bits(53) as f64 * 1.110223E-16
    }

    fn set_seed(&mut self, seed: i64, _gaussian: &mut MarsagliaPolarGaussian) {
        let (low, high) = i64_seed_to_u128_seed(seed);
        self.rng = XoroshiroPlusPlus::new(low, high)
    }
}

struct XoroshiroPlusPlus {
    seed_low: i64,
    seed_high: i64,
}

impl XoroshiroPlusPlus {
    pub fn new(low: i64, high: i64) -> Self {
        // the seed being 0 is a bad case for the rng
        // so we replace 0 seeds with the same thing vanilla uses
        if (low | high) == 0 {
            XoroshiroPlusPlus {
                seed_low: -7046029254386353131,
                seed_high: 7640891576956012809,
            }
        } else {
            XoroshiroPlusPlus {
                seed_low: low,
                seed_high: high,
            }
        }
    }

    // pub fn from_u128(seed: u128) -> Self {
    //     Self::new(seed as i64, (seed >> 64) as i64)
    // }

    pub fn next_long(&mut self) -> i64 {
        let low = self.seed_low;
        let mut high = self.seed_high;
        let output = (low.wrapping_add(high)).rotate_left(17).wrapping_add(low);
        high ^= low;
        self.seed_low = low.rotate_left(49) ^ high ^ high << 21;
        self.seed_high = high.rotate_left(28);
        output
    }
}
