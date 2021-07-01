use std::{
    fmt::{self, Debug, Display, Formatter},
    hash::{Hash, Hasher},
    ops::{Shl, Shr},
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i16,
    pub z: i32,
}

impl BlockPosition {
    pub fn from_u64(value: u64) -> Self {
        let x = (value >> 38) as i32;
        let y = (value & 0xFFF) as i16;
        let z = (value << 26 >> 38) as i32;

        BlockPosition { x, y, z }
    }

    pub fn as_u64(&self) -> u64 {
        ((self.x as u64 & 0x3FFFFFF) << 38)
            | ((self.z as u64 & 0x3FFFFFF) << 12)
            | (self.y as u64 & 0xFFF)
    }
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

    pub const fn as_block(&self) -> Self {
        match self {
            Coordinate::Block(_) => *self,
            &Coordinate::Chunk(pair) => Self::block(pair.x << 4, pair.z << 4),
            &Coordinate::Region(pair) => Self::block(pair.x << 9, pair.z << 9),
        }
    }

    pub const fn as_chunk(&self) -> Self {
        match self {
            &Coordinate::Block(pair) => Self::chunk(pair.x >> 4, pair.z >> 4),
            Coordinate::Chunk(_) => *self,
            &Coordinate::Region(pair) => Self::chunk(pair.x << 5, pair.z << 5),
        }
    }

    pub const fn as_region(&self) -> Self {
        match self {
            &Coordinate::Block(pair) => Self::region(pair.x >> 9, pair.z >> 9),
            &Coordinate::Chunk(pair) => Self::region(pair.x >> 5, pair.z >> 5),
            Coordinate::Region(_) => *self,
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

#[derive(Clone, Copy, PartialEq, Eq)]
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

impl From<Coordinate> for CoordinatePair {
    fn from(coords: Coordinate) -> Self {
        match coords {
            Coordinate::Block(pair) => pair,
            Coordinate::Chunk(pair) => pair,
            Coordinate::Region(pair) => pair,
        }
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

impl Hash for CoordinatePair {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64((self.x as u64) << 32 | self.z as u64);
    }
}
