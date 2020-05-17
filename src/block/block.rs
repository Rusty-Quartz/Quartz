use crate::data::{UnlocalizedName, StateID};

pub const BLOCK_COUNT: usize = 100;

pub struct Block {
    pub name: UnlocalizedName<'static>,
    pub default_state: StateID
}

// Define all block constants below
pub const AIR: Block = Block {
    name: UnlocalizedName::minecraft("air"),
    default_state: 0
};