use std::hash::{Hash, Hasher};
use std::cmp::{PartialEq, Eq};
use crate::block::Block;

pub const STATE_COUNT: usize = 500;

#[derive(Copy, Clone)]
pub struct BlockState {
    pub handle: &'static Block,
    pub data: StateData
}

impl Hash for BlockState {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.hash(state);
    }
}

impl PartialEq for BlockState {
    fn eq(&self, other: &Self) -> bool {
        self.data.eq(&other.data)
    }
}

impl Eq for BlockState {}

// Contains specific fields for each state variant
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateData {
    Static // Used by blocks that have no specific data
}

// Any enums needed for the state data should be defined below