use std::{
    fmt::{self, Debug, Display, Formatter},
    ops::{Shl, Shr},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i16,
    pub z: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoordinatePair {
    pub x: i32,
    pub z: i32,
}

/// Type alias for `CoordinatePair` to disambiguate between chunk coordinate pairs and region coordinate pairs.
pub type ChunkCoordinatePair = CoordinatePair;
/// Type alias for `CoordinatePair` to disambiguate between chunk coordinate pairs and region coordinate pairs.
pub type RegionCoordinatePair = CoordinatePair;

impl CoordinatePair {
    pub const fn new(x: i32, z: i32) -> Self {
        CoordinatePair { x, z }
    }
}

impl Display for CoordinatePair {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "x: {}, z: {}", self.x, self.z)
    }
}

impl Debug for CoordinatePair {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl Shl<usize> for CoordinatePair {
    type Output = CoordinatePair;

    fn shl(mut self, shift: usize) -> Self::Output {
        self.x <<= shift;
        self.z <<= shift;
        self
    }
}

impl Shr<usize> for CoordinatePair {
    type Output = CoordinatePair;

    fn shr(mut self, shift: usize) -> Self::Output {
        self.x >>= shift;
        self.z >>= shift;
        self
    }
}
