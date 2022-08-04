use qdat::world::location::BlockPosition;

use crate::world::chunk::gen::random::marsaglia_polar::MarsagliaPolarGaussian;

pub mod java;
pub mod legacy_random;
pub mod marsaglia_polar;
pub mod util;
pub mod worldgen;
pub mod xoroshiro;

/// A way to get random numbers from [Random Sources](RandomSource)
///
/// We need this because [next_gaussian](Random::next_gaussian) would violate the single mutable borrow rule
/// if we just used gaussian generators in the random sources
pub struct Random<T: RandomSource> {
    source: T,
    gaussian: MarsagliaPolarGaussian,
}

macro_rules! random_method {
     ($($name: ident ($($field: ident, $field_ty: ty),*), $return: ty,)*) => {
        $(
            pub fn $name(&mut self, $($field: $field_ty),*) -> $return {
                self.source.$name($($field),*)
            }
        )*
    };
    ($($name: ident, $return: ty,)*) => {
        $(
            pub fn $name(&mut self) -> $return {
                self.source.$name()
            }
        )*
    };
}

impl<T: RandomSource> Random<T> {
    // create helper methods to call the methods of RandomSource on Random
    random_method! {
        next_int, i32,
        next_long, i64,
        next_float, f32,
        next_double, f64,
        next_bool, bool,
        fork_positional, T::Positional,
    }

    random_method! {
        next_int_bounded (bound, u32), i32,
        next_int_in_range (max, i32, min, i32), i32,
        consume (bits, usize), (),
    }

    pub fn set_seed(&mut self, seed: i64) {
        self.source.set_seed(seed, &mut self.gaussian);
    }

    pub fn new(random_source: T) -> Self {
        Random {
            source: random_source,
            gaussian: MarsagliaPolarGaussian::new(),
        }
    }

    pub fn fork(&mut self) -> Random<T> {
        Random::new(self.source.fork())
    }

    pub fn next_gaussian(&mut self) -> f64 {
        self.gaussian.next_gaussian(&mut self.source)
    }
}


/// A source of random numbers
pub trait RandomSource {
    type Positional: PositionalRandomBuilder;

    fn fork(&mut self) -> Self;
    fn fork_positional(&mut self) -> Self::Positional;
    fn set_seed(&mut self, seed: i64, gaussian: &mut MarsagliaPolarGaussian);
    fn next_int(&mut self) -> i32;
    fn next_int_bounded(&mut self, bound: u32) -> i32;
    fn next_int_in_range(&mut self, max: i32, min: i32) -> i32 {
        self.next_int_bounded((max - min + 1) as u32) + min
    }
    fn next_long(&mut self) -> i64;
    fn next_bool(&mut self) -> bool;
    fn next_float(&mut self) -> f32;
    fn next_double(&mut self) -> f64;

    fn next_bits(&mut self, bits: i32) -> i32 {
        (self.next_long() >> (64 - bits as i64)) as i32
    }

    fn consume(&mut self, count: usize) {
        for _ in 0 .. count {
            self.next_int();
        }
    }
    fn is_legacy() -> bool {
        false
    }
}

pub trait PositionalRandomBuilder {
    type Source: RandomSource;
    fn fork_at(&self, x: i32, y: i32, z: i32) -> Self::Source;
    fn fork_at_block_pos(&self, pos: BlockPosition) -> Self::Source {
        self.fork_at(pos.x, pos.y as i32, pos.z)
    }
    fn fork_from_hash<T: ToString>(&self, input: T) -> Self::Source {
        self.fork_from_hashed_string(input.to_string())
    }
    fn fork_from_hashed_string(&self, str: impl AsRef<str>) -> Self::Source;
}

/// A [RandomSource] who's PRNG who's purpose is variable length
///
/// The core of this the [next_bits](BitRandomSource::next_bits) method which every other method is built on top of.
///
/// This is a pseudo-super trait of [RandomSource],
/// if you impl this you get an impl of RandomSource for free
pub trait BitRandomSource {
    type Positional: PositionalRandomBuilder;

    fn fork(&mut self) -> Self;
    fn fork_positional(&mut self) -> Self::Positional;
    fn set_seed(&mut self, seed: i64, gaussian: &mut MarsagliaPolarGaussian);
    fn next_bits(&mut self, bits: u8) -> i32;

    fn next_int(&mut self) -> i32 {
        self.next_bits(32)
    }

    fn next_int_bounded(&mut self, bound: u32) -> i32 {
        if bound & (bound - 1) == 0 {
            (((bound as i64).wrapping_mul(self.next_bits(31) as i64) as i64) >> 31) as i32
        } else {
            let mut j = self.next_bits(31);
            let mut k = j % bound as i32;

            while j - k + (bound as i32 - 1) < 0 {
                j = self.next_bits(31);
                k = j % bound as i32;
            }

            k
        }
    }

    fn next_long(&mut self) -> i64 {
        let lower = self.next_bits(32);
        let higher = self.next_bits(32);
        ((lower as i64) << 32) + higher as i64
    }

    fn next_bool(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    fn next_float(&mut self) -> f32 {
        self.next_bits(24) as f32 * 5.9604645E-8
    }

    fn next_double(&mut self) -> f64 {
        let higher = self.next_bits(26);
        let lower = self.next_bits(27);
        let l = ((higher as i64) << 27) + lower as i64;
        l as f64 * 1.110223E-16
    }
    fn consume(&mut self, count: usize) {
        for _ in 0 .. count {
            self.next_int();
        }
    }

    fn is_legacy() -> bool {
        false
    }
}

impl<B: BitRandomSource> RandomSource for B {
    type Positional = B::Positional;

    fn fork(&mut self) -> Self {
        self.fork()
    }

    fn fork_positional(&mut self) -> Self::Positional {
        self.fork_positional()
    }

    fn next_int(&mut self) -> i32 {
        self.next_int()
    }

    fn next_int_bounded(&mut self, bound: u32) -> i32 {
        self.next_int_bounded(bound)
    }

    fn next_long(&mut self) -> i64 {
        self.next_long()
    }

    fn next_bool(&mut self) -> bool {
        self.next_bool()
    }

    fn next_float(&mut self) -> f32 {
        self.next_float()
    }

    fn next_double(&mut self) -> f64 {
        self.next_double()
    }

    fn is_legacy() -> bool {
        Self::is_legacy()
    }

    fn next_bits(&mut self, bits: i32) -> i32 {
        self.next_bits(bits as u8)
    }

    fn set_seed(&mut self, seed: i64, gaussian: &mut MarsagliaPolarGaussian) {
        self.set_seed(seed, gaussian)
    }
}
