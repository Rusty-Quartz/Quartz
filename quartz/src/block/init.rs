use std::collections::{BTreeMap, HashMap};
use std::ptr;
use lazy_static::lazy_static;
use log::info;
use serde::{Serialize, Deserialize};
use serde_json;
use once_cell::sync::OnceCell;
use crate::block::{StateID, Block, BlockState, StateBuilder};
use util::UnlocalizedName;

static BLOCK_LIST: OnceCell<HashMap<UnlocalizedName, Block>> = OnceCell::new();
static GLOBAL_PALETTE: OnceCell<Vec<BlockState>> = OnceCell::new();

lazy_static! {
    static ref DUMMY_BLOCK: Block = Block {
        name: UnlocalizedName::minecraft("dummy"),
        properties: BTreeMap::new(),
        base_state: 0,
        default_state: 0
    };
}

#[inline(always)]
pub fn get_block_list() -> &'static HashMap<UnlocalizedName, Block> {
    BLOCK_LIST.get().expect("Block list not initialized.")
}

#[inline(always)]
pub fn get_global_palette() -> &'static Vec<BlockState> {
    GLOBAL_PALETTE.get().expect("Global palette not initialized.")
}

#[inline]
pub fn get_block(block_name: &UnlocalizedName) -> Option<&'static Block> {
    get_block_list().get(block_name)
}

#[inline]
pub fn default_state(block_name: &UnlocalizedName) -> Option<StateID> {
    get_block_list().get(block_name).map(|block| block.default_state)
}

#[inline]
pub fn get_state(id: StateID) -> Option<&'static BlockState> {
    get_global_palette().get(id as usize)
}

#[inline]
pub fn new_state(block_name: &UnlocalizedName) -> Option<StateBuilder> {
    get_block_list().get(block_name).map(|block| StateBuilder::new(&get_global_palette()[block.default_state as usize]))
}

pub fn init_blocks() {
    info!("Loading block data");

    let parsed_data = serde_json::from_str::<HashMap<String, RawBlockInfo>>(include_str!("../../../assets/blocks.json"))
        .expect("assets/blocks.json is corrupted.");

    let mut block_list: HashMap<UnlocalizedName, Block> = HashMap::with_capacity(parsed_data.len());
    let mut name_map: HashMap<String, UnlocalizedName> = HashMap::with_capacity(parsed_data.len());
    let mut largest_state: usize = 0;

    for (name, block_info) in parsed_data.iter() {
        let uln = UnlocalizedName::parse(name).expect("Invalid block name encountered during registration.");
        name_map.insert(name.clone(), uln.clone());

        // This should never happen if the data integrity is not compromised
        if block_info.states.is_empty() {
            panic!("Invalid block encountered: {}, no states found.", name);
        }

        block_list.insert(uln.clone(), Block {
            name: uln,
            properties: block_info.properties.clone(),
            base_state: block_info.states[0].id,
            default_state: block_info.default
        });

        // Use this to determine the size of the global palette
        let id = block_info.states.last().unwrap().id as usize;
        if id > largest_state {
            largest_state = id;
        }
    }

    match BLOCK_LIST.set(block_list) {
        Ok(()) => {},
        Err(_) => panic!("Block list already initialized.")
    }

    let mut global_palette: Vec<BlockState> = Vec::with_capacity(largest_state + 1);
    global_palette.resize_with(largest_state + 1, || BlockState {
        handle: &DUMMY_BLOCK,
        properties: BTreeMap::new()
    });

    for (name, block) in parsed_data {
        // All of the unwraps are guaranteed to succeed
        let handle: &'static Block = BLOCK_LIST.get().unwrap().get(name_map.get(&name).unwrap()).unwrap();
        
        for state_info in block.states {
            // Make sure we're not going out of bounds
            assert!((state_info.id as usize) < global_palette.len(), "Invalid state ID encountered: {} > {}", state_info.id, global_palette.len());

            let state = BlockState {
                handle,
                properties: state_info.properties
            };

            // Make sure the computed ID matches the ID in the generated data
            assert_eq!(state_info.id, state.id(), "Computed ID for {} does not match stored ID.", state);

            global_palette.insert(state_info.id as usize, state);
        }
    }

    match GLOBAL_PALETTE.set(global_palette) {
        Ok(()) => {},
        Err(_) => panic!("Global palette already initialized.")
    }
}

#[derive(Serialize, Deserialize)]
struct RawBlockInfo {
    // Use a BTreeMap for ordering so that we can compute state IDs
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, Vec<String>>,
    default: StateID,
    states: Vec<RawStateInfo>
}

#[derive(Serialize, Deserialize)]
struct RawStateInfo {
    id: StateID,
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, String>
}