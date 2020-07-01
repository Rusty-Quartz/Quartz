mod init;
mod state;
pub mod entity;

pub mod entities {
    pub mod furnace_entity;
    pub use furnace_entity::FurnaceBlockEntity;
}

pub use init::*;
pub use state::*;
