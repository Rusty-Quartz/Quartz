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
pub enum Coordinate {
    Block(CoordinatePair),
    Chunk(CoordinatePair),
    Region(CoordinatePair),
}

impl Coordinate {
    pub const fn block(x: i32, z: i32) -> Self {
        Self::Block(CoordinatePair::new(x, z))
    }

    pub const fn chunk(x: i32, z: i32) -> Self {
        Self::Chunk(CoordinatePair::new(x, z))
    }

    pub const fn region(x: i32, z: i32) -> Self {
        Self::Region(CoordinatePair::new(x, z))
    }

    pub const fn into_block(self) -> Self {
        match self {
            Coordinate::Block(_) => self,
            Coordinate::Chunk(pair) => Self::block(pair.x << 4, pair.z << 4),
            Coordinate::Region(pair) => Self::block(pair.x << 9, pair.z << 9),
        }
    }

    pub const fn into_chunk(self) -> Self {
        match self {
            Coordinate::Block(pair) => Self::chunk(pair.x >> 4, pair.z >> 4),
            Coordinate::Chunk(_) => self,
            Coordinate::Region(pair) => Self::chunk(pair.x << 5, pair.z << 5),
        }
    }

    pub const fn into_region(self) -> Self {
        match self {
            Coordinate::Block(pair) => Self::region(pair.x >> 9, pair.z >> 9),
            Coordinate::Chunk(pair) => Self::region(pair.x >> 5, pair.z >> 5),
            Coordinate::Region(_) => self,
        }
    }

    pub const fn into_inner(self) -> CoordinatePair {
        match self {
            Coordinate::Block(pair) => pair,
            Coordinate::Chunk(pair) => pair,
            Coordinate::Region(pair) => pair,
        }
    }
}

impl Display for Coordinate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Coordinate::Block(CoordinatePair { x, z }) => write!(f, "[Block] x: {}, z: {}", x, z),
            Coordinate::Chunk(CoordinatePair { x, z }) => write!(f, "[Chunk] x: {}, z: {}", x, z),
            Coordinate::Region(CoordinatePair { x, z }) => write!(f, "[Region] x: {}, z: {}", x, z),
        }
    }
}

impl Debug for Coordinate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Coordinate::Block(CoordinatePair { x, z }) => write!(f, "B({}, {})", x, z),
            Coordinate::Chunk(CoordinatePair { x, z }) => write!(f, "C({}, {})", x, z),
            Coordinate::Region(CoordinatePair { x, z }) => write!(f, "R({}, {})", x, z),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoordinatePair {
    pub x: i32,
    pub z: i32,
}

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
        write!(f, "({}, {})", self.x, self.z)
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
