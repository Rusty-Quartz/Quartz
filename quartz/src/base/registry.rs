use crate::block::{
    self,
    entity::{BlockEntity, StaticBlockEntity},
    states::BLOCK_LOOKUP_BY_NAME,
    Block, BlockState, StaticBlockState,
};
use log::info;
use num_traits::Num;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::convert::TryInto;
use std::fmt::{Debug, Display};
use util::UnlocalizedName;

static GLOBAL_STATIC_REGISTRY: OnceCell<StaticRegistry> = OnceCell::new();

pub type StaticStateID = <StaticRegistry as BlockRegistry>::StateID;
pub type DynamicStateID = u32;

pub trait Registry
where
    Self: BlockRegistry + BlockEntityRegistry + Sized + 'static,
{
    fn new() -> Self;

    fn global() -> &'static Self;

    fn set_global(registry: Self) -> Result<(), Self>;
}

pub trait BlockRegistry {
    type StateID: Num + Copy + Send + Sync + Display + Debug + 'static;

    type BlockState: BlockState<Self::StateID> + Sized;

    fn default_state(self: &'static Self, block_name: &UnlocalizedName)
        -> Option<Self::BlockState>;
}

pub trait BlockEntityRegistry {
    type BlockEntity: BlockEntity + Send + Sized;
}

pub struct StaticRegistry {
    blocks: &'static [Block<StaticStateID>],
}

impl BlockRegistry for StaticRegistry {
    type StateID = u16;

    type BlockState = StaticBlockState;

    fn default_state(
        self: &'static Self,
        block_name: &UnlocalizedName,
    ) -> Option<Self::BlockState> {
        if block_name.namespace != "minecraft" {
            return None;
        }

        BLOCK_LOOKUP_BY_NAME
            .get(block_name.identifier.as_str())
            .map(|meta| StaticBlockState {
                handle: &self.blocks[meta.internal_block_id],
                data: meta.default_state_data,
            })
    }
}

impl BlockEntityRegistry for StaticRegistry {
    type BlockEntity = StaticBlockEntity;
}

impl Registry for StaticRegistry {
    #[inline]
    fn new() -> Self {
        StaticRegistry {
            blocks: block::init::load_block_list::<StaticStateID>().leak(),
        }
    }

    #[inline]
    fn global() -> &'static Self {
        #[cfg(debug_assertions)]
        {
            GLOBAL_STATIC_REGISTRY
                .get()
                .expect("Global static registry not initialized")
        }

        #[cfg(not(debug_assertions))]
        {
            match GLOBAL_STATIC_REGISTRY.get() {
                Some(registry) => registry,
                None => unsafe { std::hint::unreachable_unchecked() },
            }
        }
    }

    #[inline]
    fn set_global(registry: Self) -> Result<(), Self> {
        GLOBAL_STATIC_REGISTRY.set(registry)
    }
}
