use once_cell::sync::OnceCell;
use std::collections::HashMap;
use crate::item::{Item, ItemInfo, ToolLevel, ToolType, ArmorType, UsableType, RangedWeapon};
use crate::data::UnlocalizedName;
use serde_json::from_str;
use serde::Deserialize;
use log::info;

static ITEM_LIST: OnceCell<HashMap<UnlocalizedName, Item>> = OnceCell::new();

#[inline(always)]
pub fn get_item_list() -> &'static HashMap<UnlocalizedName, Item> {
    ITEM_LIST.get().expect("Item list not initialized.")
}

#[inline]
pub fn get_item(item_name: &UnlocalizedName) -> Option<&'static Item> {
    get_item_list().get(item_name)
}

pub fn init_items() {
    info!("loading item data");

    // Load in assets/items.json generated from data-generator
    let raw_list = from_str::<HashMap<String, RawItemData>>(include_str!("../assets/items.json")).expect("items.json is corrupt");

    let mut item_list: HashMap<UnlocalizedName, Item> = HashMap::with_capacity(raw_list.len());

    for (name, raw_data) in raw_list.iter() {
        let uln = UnlocalizedName::parse(name).expect("Invalid item name in items.json");

        // This should never happen if the data integrity is not compromised 
        assert_ne!(0, raw_data.stack_size, "Item has max stack size of 0, {}", name);

        //   Determine if the item has extra info and what that info is
        let item_info = if raw_data.info.is_some() {
            match raw_data.info.as_ref().unwrap() {
                RawItemInfo::RawToolInfo { tool_type, level, attack_damage} => {
                    Some(ItemInfo::ToolInfo {
                        tool_type: ToolType::from_str(&tool_type),
                        level: ToolLevel::from_str(&level),
                        attack_damage: *attack_damage
                    })
                },

                RawItemInfo::RawFoodInfo {hunger, saturation, meat, eat_when_full, snack} => {
                    Some(ItemInfo::FoodInfo {
                        hunger: *hunger,
                        saturation: *saturation,
                        meat: *meat,
                        eat_when_full: *eat_when_full,
                        snack: *snack
                    })
                },

                RawItemInfo::RawArmorInfo {armor_type, protection, toughness, max_durability} => {
                    Some(ItemInfo::ArmorInfo {
                        armor_type: ArmorType::from_str(armor_type),
                        protection: *protection,
                        toughness: *toughness,
                        max_durability: *max_durability
                    })
                },

                RawItemInfo::RawUsableInfo {usable_type, max_durability} => {
                    Some(ItemInfo::UsableInfo {
                        usable_type: UsableType::from_str(usable_type),
                        max_durability: *max_durability
                    })
                },

                RawItemInfo::RawRangedWeaponInfo {weapon_type, max_charge_time, max_durability} => {
                    Some(ItemInfo::RangedWeaponInfo {
                        weapon_type: RangedWeapon::from_str(weapon_type),
                        max_chage_time: *max_charge_time,
                        max_durability: *max_durability
                    })
                },

                RawItemInfo::ElytraInfo {max_durability} => {
                    Some(ItemInfo::ElytraInfo {
                        max_durability: *max_durability
                    })
                }
            }
        } else { None };

        item_list.insert(uln.clone(), Item {
            id: uln,
            stack_size: raw_data.stack_size,
            rarity: raw_data.rarity,
            item_info
        });
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
    pub info: Option<RawItemInfo>
}

#[derive(Deserialize)]
enum RawItemInfo {
    RawFoodInfo {
        hunger: u32,
        saturation: f32,
        meat: bool,
        eat_when_full: bool,
        snack: bool,
        // TODO: Add status effects
        // status_effects: Vec<PotionEffect>
    },

    RawToolInfo {
        tool_type: String,
        level: String,
        attack_damage: f32
    },

    RawArmorInfo {
        armor_type: String,
        protection: u32,
        toughness: f32,
        max_durability: u32
    },

    RawUsableInfo {
        usable_type: String,
        max_durability: u32
    },

    RawRangedWeaponInfo {
        weapon_type: String,
        max_charge_time: u32,
        max_durability: u32
    },

    ElytraInfo {
        max_durability: u32
    }
}
