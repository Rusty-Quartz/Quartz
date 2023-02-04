use log::info;
use once_cell::sync::OnceCell;
use qdat::{
    item::{Item, ItemInfo},
    UlnStr,
    UnlocalizedName,
};
use serde::Deserialize;
use serde_json::from_str;
use std::collections::BTreeMap;

use crate::assets;

static ITEM_LIST: OnceCell<BTreeMap<UnlocalizedName, Item>> = OnceCell::new();

/// Gets the whole item list
#[inline(always)]
pub fn get_item_list() -> &'static BTreeMap<UnlocalizedName, Item> {
    ITEM_LIST.get().expect("Item list not initialized.")
}

/// Gets an item instance from a unlocalized name
#[inline]
pub fn get_item(item_name: &UlnStr) -> Option<&'static Item> {
    get_item_list().get(item_name)
}

/// Initializes the item list
pub fn init_items() {
    info!("Loading item data");

    // Load in assets/items.json generated from data-generator
    let raw_list = from_str::<BTreeMap<&'static str, RawItemData>>(assets::ITEM_INFO)
        .expect("items.json is corrupt");

    let mut item_list: BTreeMap<UnlocalizedName, Item> = BTreeMap::new();

    for (i, (name, raw_data)) in raw_list.into_iter().enumerate() {
        let uln = UnlocalizedName::from_str(name).expect("Invalid item name in items.json");

        // This should never happen if the data integrity is not compromised
        assert_ne!(
            0, raw_data.stack_size,
            "Item has max stack size of 0, {name}"
        );

        // NOTE: this is disabled because I don't feel like trying to make the id an &'static str
        item_list.insert(uln.clone(), Item {
            id: name,
            num_id: i as u16,
            stack_size: raw_data.stack_size,
            rarity: raw_data.rarity,
            item_info: raw_data.info,
        });
    }

    if ITEM_LIST.set(item_list).is_err() {
        panic!("ITEM_LIST already initialized.")
    }
}

// How the item info is stored in the json
#[derive(Deserialize)]
struct RawItemData {
    pub stack_size: u8,
    pub rarity: u8,
    pub info: Option<ItemInfo>,
}
