// NOTE: this file has to be updated every time we update qdat/item/item_info.rs

use quote::{quote, ToTokens};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
#[allow(clippy::enum_variant_names)]
pub enum ItemInfo {
    FoodInfo {
        hunger: u32,
        saturation: f32,
        meat: bool,
        eat_when_full: bool,
        snack: bool,
    },

    ToolInfo {
        tool_type: ToolType,
        level: ToolLevel,
        attack_damage: f32,
    },

    ArmorInfo {
        armor_type: ArmorType,
        protection: u32,
        toughness: f32,
        max_durability: u32,
    },

    UsableInfo {
        usable_type: UsableType,
        max_durability: u32,
    },

    RangedWeaponInfo {
        weapon_type: RangedWeapon,
        max_charge_time: u32,
        max_durability: u32,
    },
}

impl ToTokens for ItemInfo {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let item_tokens = match self {
            ItemInfo::ArmorInfo {
                armor_type,
                protection,
                toughness,
                max_durability,
            } => {
                quote! {
                    ItemInfo::ArmorInfo {
                        armor_type: #armor_type,
                        protection: #protection,
                        toughness: #toughness,
                        max_durability: #max_durability
                    }
                }
            }
            ItemInfo::FoodInfo {
                hunger,
                saturation,
                meat,
                eat_when_full,
                snack,
            } => {
                quote! {
                    ItemInfo::FoodInfo {
                        hunger: #hunger,
                        saturation: #saturation,
                        meat: #meat,
                        eat_when_full: #eat_when_full,
                        snack: #snack
                    }
                }
            }
            ItemInfo::RangedWeaponInfo {
                weapon_type,
                max_charge_time,
                max_durability,
            } => {
                quote! {
                    ItemInfo::RangedWeaponInfo {
                        weapon_type: #weapon_type,
                        max_charge_time: #max_charge_time,
                        max_durability: #max_durability
                    }
                }
            }
            ItemInfo::ToolInfo {
                tool_type,
                level,
                attack_damage,
            } => {
                quote! {
                    ItemInfo::ToolInfo {
                        tool_type: #tool_type,
                        level: #level,
                        attack_damage: #attack_damage
                    }
                }
            }
            ItemInfo::UsableInfo {
                usable_type,
                max_durability,
            } => {
                quote! {
                    ItemInfo::UsableInfo {
                        usable_type: #usable_type,
                        max_durability: #max_durability
                    }
                }
            }
        };

        tokens.extend(item_tokens);
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Sword,
    Pickaxe,
    Shovel,
    Axe,
    Hoe,
}

impl ToTokens for ToolType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            ToolType::Sword => quote! {ToolType::Sword},
            ToolType::Pickaxe => quote! {ToolType::Pickaxe},
            ToolType::Shovel => quote! {ToolType::Shovel},
            ToolType::Axe => quote! {ToolType::Axe},
            ToolType::Hoe => quote! {ToolType::Hoe},
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLevel {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    Netherite,
}

impl ToTokens for ToolLevel {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            ToolLevel::Wood => quote! {ToolLevel::Wood},
            ToolLevel::Stone => quote! {ToolLevel::Stone},
            ToolLevel::Iron => quote! {ToolLevel::Iron},
            ToolLevel::Gold => quote! {ToolLevel::Gold},
            ToolLevel::Diamond => quote! {ToolLevel::Diamond},
            ToolLevel::Netherite => quote! {ToolLevel::Netherite},
        })
    }
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorType {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

impl ToTokens for ArmorType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            ArmorType::Helmet => quote! {ArmorType::Helmet},
            ArmorType::Chestplate => quote! {ArmorType::Chestplate},
            ArmorType::Leggings => quote! {ArmorType::Leggings},
            ArmorType::Boots => quote! {ArmorType::Boots},
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsableType {
    Shears,
    FishingRod,
    FlintAndSteel,
    Shield,
    CarrotStick,
    FungusStick,
}

impl ToTokens for UsableType {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            UsableType::Shears => quote! {UsableType::Shears},
            UsableType::FishingRod => quote! {UsableType::FishingRod},
            UsableType::FlintAndSteel => quote! {UsableType::FlintAndSteel},
            UsableType::Shield => quote! {UsableType::Sheld},
            UsableType::CarrotStick => quote! {UsableType::CarrotStick},
            UsableType::FungusStick => quote! {UsableType::FungusStick},
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangedWeapon {
    Bow,
    Crossbow,
    Trident,
}

impl ToTokens for RangedWeapon {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        tokens.extend(match self {
            RangedWeapon::Bow => quote! {RangedWeapon::Bow},
            RangedWeapon::Crossbow => quote! {RangedWeapon::Crossbow},
            RangedWeapon::Trident => quote! {RangedWeapon::Trident},
        })
    }
}
