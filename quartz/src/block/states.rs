include!(concat!(env!("OUT_DIR"), "/blockstate_output.rs"));

pub(crate) struct BlockStateMetadata {
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
