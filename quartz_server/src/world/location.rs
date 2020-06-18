#[derive(PartialEq, Eq, Hash)]
pub struct BlockPosition {
    pub x: i32,
    pub y: i16,
    pub z: i32
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoordinatePair {
    pub x: i32,
    pub z: i32
}

impl CoordinatePair {
    pub fn scaled_up(&self, factor: i32) -> Self {
        CoordinatePair {
            x: self.x * factor,
            z: self.z * factor
        }
    }
}