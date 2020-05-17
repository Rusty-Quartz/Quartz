use std::collections::HashMap;

use crate::block::{Block, BlockState};
use crate::data::UnlocalizedName;

pub type StateID = u16;

pub struct Registry {
    blocks: BlockRegistry
}

impl Registry {
    pub fn new(
        block_count: usize,
        state_count: usize
    ) -> Self {
        Registry {
            blocks: BlockRegistry::new(block_count, state_count)
        }
    }

    pub fn register_block(&mut self, block: &'static Block) {
        self.blocks.block_list.insert(block.name.clone(), block);
    }

    pub fn register_state(&mut self, id: StateID, state: BlockState) {
        // Make sure the ID is within bounds
        assert!(
            (id as usize) < self.blocks.global_palette.len(),
            "Invalid state ID encountered during registration: {} > {}",
            id, self.blocks.global_palette.len()
        );

        self.blocks.global_palette[id as usize] = state;
        self.blocks.state_id_map.insert(state, id);
    }

    pub fn get_block(&self, name: &UnlocalizedName) -> Option<&'static Block> {
        self.blocks.block_list.get(name).cloned()
    }
}

struct BlockRegistry {
    // Maps a block's unlocalized name to its default state ID
    block_list: HashMap<UnlocalizedName<'static>, &'static Block>,

    // Maps a state ID (index) to a block state
    global_palette: Vec<BlockState>,

    // Maps a constructed block state to its ID
    state_id_map: HashMap<BlockState, StateID>
}

impl BlockRegistry {
    pub fn new(block_count: usize, state_count: usize) -> Self {
        let mut global_palette: Vec<BlockState> = Vec::with_capacity(state_count);

        // This is safe because we set the capacity of the vec above
        unsafe {
            global_palette.set_len(state_count);
        }

        BlockRegistry {
            block_list: HashMap::with_capacity(block_count),
            global_palette,
            state_id_map: HashMap::with_capacity(state_count)
        }
    }
}