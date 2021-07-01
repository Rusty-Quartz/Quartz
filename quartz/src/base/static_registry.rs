use crate::{
    block::{
        entity::StaticBlockEntity,
        init::*,
        states::BLOCK_LOOKUP_BY_NAME,
        Block,
        StaticBlockState,
    },
    command::StaticCommandExecutor,
};
use log::info;
use once_cell::sync::OnceCell;
use quartz_util::UnlocalizedName;

static GLOBAL_STATIC_REGISTRY: OnceCell<StaticRegistry> = OnceCell::new();

pub type BlockState = StaticBlockState;
pub type StateID = u16;
pub type BlockEntity = StaticBlockEntity;
pub type Registry = StaticRegistry;
pub type CommandExecutor = StaticCommandExecutor;

pub struct StaticRegistry {
    blocks: &'static [Block],
    global_palette: Box<[StaticBlockState]>,
}

impl StaticRegistry {
    pub(crate) fn init() -> Result<(), ()> {
        info!("Initializing static registry");

        info!("Initializing blocks");
        let mut raw = load_raw_block_data();
        attach_behavior(&mut raw);
        let blocks = make_block_list(&raw).leak();
        let global_palette = make_static_global_palette(&raw, blocks).into_boxed_slice();

        GLOBAL_STATIC_REGISTRY
            .set(StaticRegistry {
                blocks,
                global_palette,
            })
            .map_err(|_| ())
    }

    #[inline]
    pub fn get() -> &'static Self {
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

    pub fn default_state(block_name: &UnlocalizedName) -> Option<BlockState> {
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

    pub fn state_for_id(id: StateID) -> Option<&'static BlockState> {
        Self::get().global_palette.get(id as usize)
    }
}
