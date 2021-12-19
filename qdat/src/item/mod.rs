// We allow module name to be the same because we don't expose the item module
#[allow(clippy::module_inception)]
mod item;
#[allow(missing_docs)]
mod item_info;

pub use item::*;
pub use item_info::*;

use crate::block::states::BlockStateData;
pub fn item_to_block(item: &'static Item) -> Option<BlockStateData> {
    crate::block::states::BLOCK_LOOKUP_BY_NAME
        .get(item.id)
        .map(|bsm| bsm.default_state_data)
}
