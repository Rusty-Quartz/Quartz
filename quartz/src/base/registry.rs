use crate::block::{
    entity::{BlockEntity, StaticBlockEntity},
    init::*,
    states::BLOCK_LOOKUP_BY_NAME,
    Block,
    BlockState,
    StaticBlockState,
};
use log::info;
use num_traits::Num;
use once_cell::sync::OnceCell;
use std::fmt::{Debug, Display};
use util::UnlocalizedName;

static GLOBAL_STATIC_REGISTRY: OnceCell<StaticRegistry> = OnceCell::new();

pub type StaticStateID = <StaticRegistry as BlockRegistry<StaticRegistry>>::StateID;
pub type DynamicStateID = u32;

pub trait Registry: BlockRegistry<Self> + BlockEntityRegistry + Sized + 'static {
    fn new() -> Self;

    fn set_global(registry: Self) -> Result<(), Self>;
}

pub trait BlockRegistry<R: Registry>: Sized {
    type StateID: Num + Copy + Send + Sync + Display + Debug + 'static;

    type BlockState: BlockState<R> + Sized;

    fn default_state(block_name: &UnlocalizedName) -> Option<Self::BlockState>;

    fn state_for_id(id: Self::StateID) -> Option<&'static Self::BlockState>;
}

pub trait BlockEntityRegistry {
    type BlockEntity: BlockEntity + Send + Sized;
}

pub struct StaticRegistry {
    blocks: &'static [Block<Self>],
    global_palette: Box<[StaticBlockState]>,
}

impl StaticRegistry {
    #[inline]
    fn get() -> &'static Self {
        #[cfg(debug_assertions)]
        {
            GLOBAL_STATIC_REGISTRY
                .get()
                .expect("Global static registry not initialized")
        }

        #[cfg(not(debug_assertions))]
        {
            unsafe { GLOBAL_STATIC_REGISTRY.get_unchecked() }
        }
    }
}

impl BlockRegistry<Self> for StaticRegistry {
    type BlockState = StaticBlockState;
    type StateID = u16;

    fn default_state(block_name: &UnlocalizedName) -> Option<Self::BlockState> {
        if block_name.namespace != "minecraft" {
            return None;
        }

        BLOCK_LOOKUP_BY_NAME
            .get(block_name.identifier.as_str())
            .map(|meta| StaticBlockState {
                // Safety: internal block IDs are guaranteed to be consistent and in-bounds
                handle: unsafe { &Self::get().blocks.get_unchecked(meta.internal_block_id) },
                data: meta.default_state_data,
            })
    }

    fn state_for_id(id: Self::StateID) -> Option<&'static Self::BlockState> {
        Self::get().global_palette.get(id as usize)
    }
}

impl BlockEntityRegistry for StaticRegistry {
    type BlockEntity = StaticBlockEntity;
}

impl Registry for StaticRegistry {
    #[inline]
    fn new() -> Self {
        info!("Initializing static registry");

        info!("Initializing blocks");
        let mut raw = load_raw_block_data::<Self>();
        attach_behavior(&mut raw);
        let blocks = make_block_list(&raw).leak();
        let global_palette = make_static_global_palette(&raw, blocks).into_boxed_slice();

        StaticRegistry {
            blocks,
            global_palette,
        }
    }

    #[inline]
    fn set_global(registry: Self) -> Result<(), Self> {
        GLOBAL_STATIC_REGISTRY.set(registry)
    }
}
