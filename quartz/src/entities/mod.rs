use qdat::world::location::{BlockPosition, Coordinate};

pub mod player;


pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<BlockPosition> for Position {
    fn from(coord: BlockPosition) -> Self {
        Self {
            x: coord.x as f64,
            y: coord.y as f64,
            z: coord.z as f64,
        }
    }
}
