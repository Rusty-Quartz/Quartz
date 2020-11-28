use crate::{
    base::{
        assets,
        registry::{DynamicStateID, StaticStateID},
    },
    block::{states::BLOCK_LOOKUP_BY_NAME, *},
};
use itertools::Itertools;
use log::info;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};
use tinyvec::ArrayVec;
use util::UnlocalizedName;

static BLOCK_LIST: OnceCell<HashMap<UnlocalizedName, Block<DynamicStateID>>> = OnceCell::new();
static GLOBAL_PALETTE: OnceCell<Box<[DynamicBlockState]>> = OnceCell::new();

#[inline(always)]
pub fn get_block_list() -> &'static HashMap<UnlocalizedName, Block<DynamicStateID>> {
    BLOCK_LIST.get().expect("Block list not initialized.")
}

#[inline(always)]
pub fn get_global_palette() -> &'static [DynamicBlockState] {
    GLOBAL_PALETTE
        .get()
        .expect("Global palette not initialized.")
}

#[inline]
pub fn get_block(block_name: &UnlocalizedName) -> Option<&'static Block<DynamicStateID>> {
    get_block_list().get(block_name)
}

#[inline]
pub fn default_state(block_name: &UnlocalizedName) -> Option<DynamicStateID> {
    get_block_list()
        .get(block_name)
        .map(|block| block.default_state)
}

#[inline]
pub fn get_state(id: DynamicStateID) -> Option<&'static DynamicBlockState> {
    get_global_palette().get(id as usize)
}

#[inline]
pub fn new_state(block_name: &UnlocalizedName) -> Option<DynamicStateBuilder> {
    get_block_list()
        .get(block_name)
        .map(|block| DynamicStateBuilder::new(&get_global_palette()[block.default_state as usize]))
}

pub(crate) fn load_raw_block_data<'de, T: Deserialize<'de>>() -> HashMap<String, RawBlockInfo<T>> {
    serde_json::from_str::<HashMap<String, RawBlockInfo<T>>>(assets::BLOCK_INFO)
        .expect("assets/blocks.json is corrupted.")
}

pub(crate) fn make_block_list<T: Copy>(raw: &HashMap<String, RawBlockInfo<T>>) -> Vec<Block<T>> {
    let mut block_list = Vec::new();

    for (name, block_info) in raw.into_iter().sorted_by_key(|(_, info)| info.interm_id) {
        let uln = UnlocalizedName::from_str(&name)
            .expect("Invalid block name encountered during registration.");

        // This should never happen if the data integrity is not compromised
        if block_info.states.is_empty() {
            panic!("Invalid block encountered: {}, no states found.", name);
        }

        block_list.push(Block {
            name: uln,
            properties: block_info
                .properties
                .clone()
                .into_iter()
                .collect::<ArrayVec<_>>(),
            base_state: block_info.states[0].id,
            default_state: block_info.default,
        });
    }

    block_list
}

pub(crate) fn make_static_global_palette(
    raw: &HashMap<String, RawBlockInfo<StaticStateID>>,
    blocks: &'static [Block<StaticStateID>],
) -> Vec<StaticBlockState>
{
    let mut global_palette = Vec::new();

    for (_, block) in raw {
        let handle: &'static Block<StaticStateID> = &blocks[block.interm_id];

        for state_info in block.states.iter() {
            let default_state = StaticBlockState {
                handle,
                data: BLOCK_LOOKUP_BY_NAME
                    .get(handle.name.identifier.as_str())
                    .unwrap()
                    .default_state_data,
            };
            let mut state_builder = StaticStateBuilder::new(default_state.clone());

            for (key, value) in state_info.properties.iter() {
                state_builder
                    .add_property(key.as_str(), value.as_str())
                    .unwrap();
            }

            let state = state_builder.build();

            // Make sure the computed ID matches the ID in the generated data
            assert_eq!(
                state_info.id,
                state.id(),
                "Computed ID for {} does not match stored ID.",
                state.handle.name
            );

            global_palette.push(state);
        }
    }

    global_palette.sort_by_key(|state| state.id());
    global_palette
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RawBlockInfo<T> {
    // Use a BTreeMap for ordering so that we can compute state IDs
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, Vec<String>>,
    default: T,
    interm_id: usize,
    states: Vec<RawStateInfo<T>>,
}

#[derive(Serialize, Deserialize)]
struct RawStateInfo<T> {
    id: T,
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, String>,
}
