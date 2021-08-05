pub mod entity;
mod init;
pub mod state;
pub(crate) use init::*;
pub use state::*;

#[allow(missing_docs)]
pub mod entities {
    pub mod furnace_entity;
    pub use furnace_entity::FurnaceBlockEntity;
}
