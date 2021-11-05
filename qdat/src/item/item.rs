use crate::item::*;

include!(concat!(env!("OUT_DIR"), "/items_output.rs"));

/// Represents a minecraft item
#[derive(Debug)]
pub struct Item {
    /// The item id
    pub id: &'static str,
    pub num_id: u16,
    /// The max size a stack can be
    pub stack_size: u8,
    /// The rarity of the item
    pub rarity: u8,
    /// Holds extra info about the item
    pub item_info: Option<ItemInfo>,
}
