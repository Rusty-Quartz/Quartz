use std::sync::atomic::{AtomicI64, Ordering};

use crate::world::chunk::gen::random::{
    marsaglia_polar::MarsagliaPolarGaussian,
    util::{get_pos_seed, hash_string_md5, java_string_hash},
    BitRandomSource,
    PositionalRandomBuilder,
};

const MODULUS_BITS: i32 = 48;
const MODULUS_MASK: i64 = 281474976710655;
const MULTIPLIER: i64 = 25214903917;
const INCREMENT: i64 = 11;

pub struct LegacyRandom {
    seed: AtomicI64,
    guassian_source: MarsagliaPolarGaussian,
}

impl LegacyRandom {
    pub fn new(seed: i64) -> LegacyRandom {
        LegacyRandom {
            seed: AtomicI64::new(seed),
            guassian_source: MarsagliaPolarGaussian::new(),
        }
    }
}

impl BitRandomSource for LegacyRandom {
    type Positional = LegacyPositionalRandom;

    fn set_seed(&mut self, seed: i64) {
        self.seed
            .store((seed ^ MULTIPLIER) & MODULUS_MASK, Ordering::Relaxed);
        self.guassian_source.reset();
    }

    fn next_bits(&mut self, bits: u8) -> i32 {
        loop {
            let orig_seed = self.seed.load(Ordering::Relaxed);
            let new_seed = (orig_seed * MULTIPLIER + INCREMENT) & MODULUS_MASK;
            if self
                .seed
                .compare_exchange(orig_seed, new_seed, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return (new_seed >> (48 - bits)) as i32;
            }
        }
    }

    fn fork(&mut self) -> Self {
        LegacyRandom::new(self.next_long())
    }

    fn fork_positional(&mut self) -> Self::Positional {
        LegacyPositionalRandom {
            seed: self.next_long(),
        }
    }

    fn is_legacy() -> bool {
        true
    }
}

pub struct LegacyPositionalRandom {
    seed: i64,
}

impl PositionalRandomBuilder for LegacyPositionalRandom {
    type Source = LegacyRandom;

    fn fork_at(&self, x: i32, y: i32, z: i32) -> Self::Source {
        let positional_seed = get_pos_seed(x, y, z);
        let new_seed = positional_seed ^ self.seed;
        LegacyRandom::new(new_seed)
    }

    fn fork_from_hashed_string(&self, str: String) -> Self::Source {
        let hash = java_string_hash(&str);
        LegacyRandom::new(hash as i64 ^ self.seed)
    }
}
