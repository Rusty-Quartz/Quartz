pub(crate) mod init;
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
    use util::UnlocalizedName;

    pub(crate) struct BlockStateMetadata {
        pub default_state_data: BlockStateData,
        pub internal_block_id: usize
    }
    
    impl BlockStateMetadata {
        const fn new(default_state_data: BlockStateData, internal_block_id: usize) -> Self {
            BlockStateMetadata {
                default_state_data,
                internal_block_id
            }
        }
    }

    include!(concat!(env!("OUT_DIR"), "/blockstate_output.rs"));
}

pub use init::*;
pub use state::*;
