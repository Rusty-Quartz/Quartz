use crate::block::StateID;

include!(concat!(env!("OUT_DIR"), "/blockstate_output.rs"));

pub const AIR: StateID = BlockStateData::Air.id();
pub const VOID_AIR: StateID = BlockStateData::VoidAir.id();
pub const CAVE_AIR: StateID = BlockStateData::CaveAir.id();

#[inline]
pub const fn is_air(state: StateID) -> bool {
    state == AIR || state == VOID_AIR || state == CAVE_AIR
}

pub struct BlockStateMetadata {
    pub default_state_data: BlockStateData,
    pub internal_block_id: usize,
}

impl BlockStateMetadata {
    const fn new(default_state_data: BlockStateData, internal_block_id: usize) -> Self {
        BlockStateMetadata {
            default_state_data,
            internal_block_id,
        }
    }
}
