use crate::{
    base::assets,
    item::{Item, ItemInfo},
};
use log::info;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use serde_json::from_str;
use std::collections::HashMap;
use std::str::FromStr;
use util::UnlocalizedName;

static ITEM_LIST: OnceCell<HashMap<UnlocalizedName, Item>> = OnceCell::new();

/// Gets the whole item list
#[inline(always)]
pub fn get_item_list() -> &'static HashMap<UnlocalizedName, Item> {
    ITEM_LIST.get().expect("Item list not initialized.")
}

/// Gets an item instance from a unlocalized name
#[inline]
pub fn get_item(item_name: &UnlocalizedName) -> Option<&'static Item> {
    get_item_list().get(item_name)
}

/// Initializes the item list
pub fn init_items() {
    info!("Loading item data");

    // Load in assets/items.json generated from data-generator
    let raw_list =
        from_str::<HashMap<String, RawItemData>>(assets::ITEM_INFO).expect("items.json is corrupt");

    let mut item_list: HashMap<UnlocalizedName, Item> = HashMap::with_capacity(raw_list.len());

    for (name, raw_data) in raw_list {
        let uln = UnlocalizedName::from_str(&name).expect("Invalid item name in items.json");

        // This should never happen if the data integrity is not compromised
        assert_ne!(
            0, raw_data.stack_size,
            "Item has max stack size of 0, {}",
            name
        );

        item_list.insert(
            uln.clone(),
            Item {
                id: uln,
                stack_size: raw_data.stack_size,
                rarity: raw_data.rarity,
                item_info: raw_data.info,
            },
        );
    }

    match ITEM_LIST.set(item_list) {
        Err(_) => panic!("ITEM_LIST already initialized."),
        _ => {}
    }
}

// How the item info is stored in the json
#[derive(Deserialize)]
struct RawItemData {
    pub stack_size: u8,
    pub rarity: u8,
    pub info: Option<ItemInfo>,
}
