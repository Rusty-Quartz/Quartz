use serde::Deserialize;

#[derive(Debug, Deserialize)]
// represents all possible extra data we could need to store about items
pub enum ItemInfo {
    FoodInfo {
        hunger: u32,
        saturation: f32,
        meat: bool,
        eat_when_full: bool,
        snack: bool,
        // TODO: Add status effects
        // status_effects: Vec<PotionEffect>
    },
    
    ToolInfo {
        tool_type: ToolType,
        level: ToolLevel,
        attack_damage: f32
        // TODO: implements enchantments
        // possible_enchantments: Vec<Enchantments>
    },

    ArmorInfo {
        armor_type: ArmorType,
        // level: ArmorLevel,
        protection: u32,
        toughness: f32,
        max_durability: u32,
        // TODO: implements enchantments
        // possible_enchantments: Vec<Enchantments>
    },

    UsableInfo {
        usable_type: UsableType,
        max_durability: u32
    },

    RangedWeaponInfo {
        weapon_type: RangedWeapon,
        max_chage_time: u32,
        max_durability: u32
    },
}

impl ItemInfo {
    pub fn max_durability(&self) -> u32 {
        match self {
            ItemInfo::FoodInfo {..} => 0,
            ItemInfo::ArmorInfo {max_durability, ..} => *max_durability,
            ItemInfo::UsableInfo {max_durability, ..} => *max_durability,
            ItemInfo::RangedWeaponInfo {max_durability , ..} => *max_durability,
            ItemInfo::ToolInfo {level, ..} => level.max_durability()
        }
    }
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    Sword,
    Pickaxe,
    Shovel,
    Axe,
    Hoe
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLevel {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    Netherite
}

impl ToolLevel {
    pub fn max_durability(&self) -> u32 {
        match self {
            ToolLevel::Wood => 59,
            ToolLevel::Gold => 32,
            ToolLevel::Stone => 131,
            ToolLevel::Iron => 250,
            ToolLevel::Diamond => 1561,
            ToolLevel::Netherite => 2031
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArmorType {
    Helmet,
    Chestplate,
    Leggings,
    Boots
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsableType {
    Shears,
    FishingRod,
    FlintAndSteel,
    Shield,
    CarrotStick,
    FungusStick
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RangedWeapon {
    Bow,
    Crossbow,
    Trident
}
