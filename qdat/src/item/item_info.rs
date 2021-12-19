use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
/// Represents all possible extra data we need to store about items
pub enum ItemInfo {
    /// The info needed for food items
    FoodInfo {
        /// The amount of hunger the item restores
        hunger: u32,
        /// The amount of saturation given
        saturation: f32,
        /// Weather the item is a meat item
        meat: bool,
        /// If the player can eat it while they are full
        eat_when_full: bool,
        /// If the player eats the food item faster than normal
        snack: bool,
        // TODO: Add status effects
        // status_effects: Vec<PotionEffect>
    },

    /// The info needed for tools
    ToolInfo {
        /// The type of the tool
        tool_type: ToolType,
        /// The level of the tool
        level: ToolLevel,
        /// The damage done by the tool
        attack_damage: f32, // TODO: implements enchantments
                            // possible_enchantments: Vec<Enchantments>
    },

    /// The info needed for armor items
    ArmorInfo {
        /// The type of the armor
        armor_type: ArmorType,
        // level: ArmorLevel,
        /// The protection given by the armor piece
        protection: u32,
        /// The amount of toughness given
        toughness: f32,
        /// The max durability of the item
        max_durability: u32,
        // TODO: implements enchantments
        // possible_enchantments: Vec<Enchantments>
    },

    /// Miscellaneous tools
    UsableInfo {
        /// Which tool type the item is
        usable_type: UsableType,
        /// The durability of the item
        max_durability: u32,
    },

    /// Ranged weapon info
    RangedWeaponInfo {
        /// Which ranged weapon it is
        weapon_type: RangedWeapon,
        /// The max charge time
        max_charge_time: u32,
        /// The max durability
        max_durability: u32,
    },
}

impl ItemInfo {
    /// Gets the max durability of an item
    pub const fn max_durability(&self) -> u32 {
        match self {
            ItemInfo::FoodInfo { .. } => 0,
            ItemInfo::ArmorInfo { max_durability, .. } => *max_durability,
            ItemInfo::UsableInfo { max_durability, .. } => *max_durability,
            ItemInfo::RangedWeaponInfo { max_durability, .. } => *max_durability,
            ItemInfo::ToolInfo { level, .. } => level.max_durability(),
        }
    }
}

/// The different types of tools
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Sword,
    Pickaxe,
    Shovel,
    Axe,
    Hoe,
}

/// The possible levels for tools
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

impl ToolLevel {
    /// The max durability of the tool based on its level
    pub const fn max_durability(&self) -> u32 {
        match self {
            ToolLevel::Wood => 59,
            ToolLevel::Gold => 32,
            ToolLevel::Stone => 131,
            ToolLevel::Iron => 250,
            ToolLevel::Diamond => 1561,
            ToolLevel::Netherite => 2031,
        }
    }
}

/// The possible armor pieces
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorType {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

/// The possible usable items
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

/// The possible ranged weapons
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangedWeapon {
    Bow,
    Crossbow,
    Trident,
}
