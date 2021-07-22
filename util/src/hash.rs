use std::{
    hash::{BuildHasher, Hasher},
    mem,
};

/// The purpose of this hasher is to be extremely fast for hashing primitive integer types containing
/// less than or equal to 64 bits. This hasher should not be used in any other context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumHasher;

impl BuildHasher for NumHasher {
    type Hasher = NumHashIsomorphism;

    fn build_hasher(&self) -> Self::Hasher {
        NumHashIsomorphism { state: 0 }
    }
}

/// This hasher is the fastest possible implementation of a hasher for the primitive integer types
/// containing no more than 64 bits. It treats values of those types as their own hash, hence the
/// hash isomorphism. For all types larger than (or potentially larger than) `u64`, the byte
/// representation of those types is xor-ed into the internal state in 64-bit chunks.
pub struct NumHashIsomorphism {
    state: u64,
}

impl Hasher for NumHashIsomorphism {
    fn finish(&self) -> u64 {
        self.state
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut buf = [0u8; 8];
        for window in bytes.windows(8) {
            (&mut buf[.. window.len()]).copy_from_slice(window);
            (window.len() .. 8).for_each(|j| buf[j] = 0);
            self.state ^= unsafe { mem::transmute_copy::<_, u64>(&buf) };
        }
    }

    fn write_i8(&mut self, i: i8) {
        self.state = i as u64;
    }

    fn write_u8(&mut self, i: u8) {
        self.state = i as u64;
    }

    fn write_i16(&mut self, i: i16) {
        self.state = i as u64;
    }

    fn write_u16(&mut self, i: u16) {
        self.state = i as u64;
    }

    fn write_i32(&mut self, i: i32) {
        self.state = i as u64;
    }

    fn write_u32(&mut self, i: u32) {
        self.state = i as u64;
    }

    fn write_i64(&mut self, i: i64) {
        self.state = i as u64;
    }

    fn write_u64(&mut self, i: u64) {
        self.state = i;
    }
}
