use crate::{
    base::{assets, StateID},
    block::{
        behavior::{BlockBehaviorSMT, DefaultBehavior},
        states::BLOCK_LOOKUP_BY_NAME,
        *,
    },
};
use itertools::Itertools;
use quartz_util::UnlocalizedName;
use serde::{Deserialize, Serialize};
use serde_json;
use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
};
use tinyvec::ArrayVec;

pub(crate) fn load_raw_block_data<'de>() -> HashMap<String, RawBlockInfo> {
    serde_json::from_str::<HashMap<String, RawBlockInfo>>(assets::BLOCK_INFO)
        .expect("assets/blocks.json is corrupted.")
}

pub(crate) fn attach_behavior(raw: &mut HashMap<String, RawBlockInfo>) {
    macro_rules! attach {
        ($behavior:ty, $( $block_name:literal ),+) => {
            $(
                raw.get_mut(concat!("minecraft:", $block_name))
                    .expect("Invalid block name during behavior attachment")
                    .behavior = Some(BlockBehaviorSMT::new::<$behavior>());
            )+
        };
    }

    attach!(DefaultBehavior, "air", "stone");
}

pub(crate) fn make_block_list(raw: &HashMap<String, RawBlockInfo>) -> Vec<Block> {
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
            behavior: block_info
                .behavior
                .clone()
                .unwrap_or(BlockBehaviorSMT::new::<DefaultBehavior>()),
        });
    }

    block_list
}

// TODO: put behind feature flag if necessary
pub(crate) fn make_static_global_palette(
    raw: &HashMap<String, RawBlockInfo>,
    blocks: &'static [Block],
) -> Vec<StaticBlockState> {
    let mut global_palette = Vec::new();

    for (_, block) in raw {
        let handle: &'static Block = &blocks[block.interm_id];

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
                "Computed ID for {} does not match stored ID.\nState is {:?}",
                state.handle.name,
                state.data
            );

            global_palette.push(state);
        }
    }

    global_palette.sort_by_key(|state| state.id());
    global_palette
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RawBlockInfo {
    // Use a BTreeMap for ordering so that we can compute state IDs
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, Vec<String>>,
    default: StateID,
    interm_id: usize,
    states: Vec<RawStateInfo<StateID>>,
    #[serde(skip_serializing, skip_deserializing, default = "Option::default")]
    behavior: Option<BlockBehaviorSMT>,
}

#[derive(Serialize, Deserialize)]
struct RawStateInfo<T> {
    id: T,
    #[serde(default = "BTreeMap::new")]
    properties: BTreeMap<String, String>,
}
