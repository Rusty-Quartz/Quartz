#![feature(arbitrary_enum_discriminant)]
mod init;
mod state;
#[allow(missing_docs)]
pub mod entity;

#[allow(missing_docs)]
pub mod entities {
    pub mod furnace_entity;
    pub use furnace_entity::FurnaceBlockEntity;
}

#[allow(missing_docs, nonstandard_style, dead_code)]
pub mod states {
    include!(concat!(env!("OUT_DIR"), "/blockstate_output.rs"));
}

pub use init::*;
pub use state::*;
