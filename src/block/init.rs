use std::collections::HashMap;
use log::info;
use crate::data::{Registry, StateID};
use crate::block::{block, StateData};

pub fn init_blocks(registry: &mut Registry) {
    info!("Loading block data");

    // Register each block
    registry.register_block(&block::AIR);

    // Register each state
}

struct RawBlockInfo {
    properties: Option<HashMap<String, Vec<String>>>,
    default: StateID,
    states: Vec<RawStateInfo>
}

struct RawStateInfo {
    id: StateID,
    properties: Option<StateData>
}