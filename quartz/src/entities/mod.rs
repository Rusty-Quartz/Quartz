use qdat::world::location::{BlockPosition, Coordinate};

pub mod player;


pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<BlockPosition> for Position {
    fn from(coord: BlockPosition) -> Self {
        Self {
            x: coord.x as f32,
            y: coord.y as f32,
            z: coord.z as f32,
        }
    }
}
