use std::fmt::Display;

#[derive(Debug)]
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

    ElytraInfo {
        max_durability: u32
    }
}

impl ItemInfo {
    pub fn max_durability(&self) -> u32 {
        match self {
            ItemInfo::FoodInfo {..} => 0,
            ItemInfo::ArmorInfo {max_durability, ..} => *max_durability,
            ItemInfo::UsableInfo {max_durability, ..} => *max_durability,
            ItemInfo::RangedWeaponInfo {max_durability , ..} => *max_durability,
            ItemInfo::ElytraInfo {max_durability,} => *max_durability,
            ItemInfo::ToolInfo {level, ..} => level.max_durability()
        }
    }
}


#[derive(Debug)]
pub enum ToolType {
    Sword,
    Pickaxe,
    Shovel,
    Axe,
    Hoe
}

impl ToolType {
    pub fn from_str(type_str: &str) -> Self {
        match type_str {
            "sword" =>ToolType::Sword,
            "pickaxe" => ToolType::Pickaxe,
            "shovel" => ToolType::Shovel,
            "axe" => ToolType::Axe,
            "hoe" => ToolType::Hoe,
            _ => panic!("Unkown tool level {}", type_str)
        }
    }
}

#[derive(Debug)]
pub enum ToolLevel {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    Netherite
}

impl ToolLevel {
    pub fn from_str(level: &str) -> Self {
        match level {
            "wood" => ToolLevel::Wood,
            "stone" => ToolLevel::Stone,
            "iron" => ToolLevel::Iron,
            "gold" => ToolLevel::Gold,
            "diamond" => ToolLevel::Diamond,
            "netherite" => ToolLevel::Netherite,
            _ => panic!("Unknown tool level {}", level)
        }
    }

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

#[derive(Debug)]
pub enum ArmorType {
    Helmet,
    Chestplate,
    Leggings,
    Boots
}

impl ArmorType {
    pub fn from_str(armor_type: &str) -> Self {
        match armor_type {
            "helmet" => ArmorType::Helmet,
            "chestplate" => ArmorType::Chestplate,
            "leggings" => ArmorType::Leggings,
            "boots" => ArmorType::Boots,
            _ => panic!("Unkown armor type {}", armor_type)
        }
    }
}

#[derive(Debug)]
pub enum UsableType {
    Shears,
    FishingRod,
    FlintAndSteel,
    Shield,
    CarrotStick,
    FungusStick
}


impl UsableType {
    pub fn from_str(usable_type: &str) -> Self {
        match usable_type {
            "shears" => UsableType::Shears,
            "fishing_rod" => UsableType::FishingRod,
            "flint_and_steel" => UsableType::FlintAndSteel,
            "shield" => UsableType::Shield,
            "carrot_stick" => UsableType::CarrotStick,
            "fungus_stick" => UsableType::FungusStick,
            _ => panic!("Unkown usable type {}", usable_type)
        }
    }
}

#[derive(Debug)]
pub enum RangedWeapon {
    Bow,
    Crossbow,
    Trident
}

impl RangedWeapon {
    pub fn from_str(weapon_type: &str) -> Self {
        match weapon_type {
            "bow" => RangedWeapon::Bow,
            "crossbow" => RangedWeapon::Crossbow,
            "trident" => RangedWeapon::Trident,
            _ => panic!("Unkown ranged weapon type {}", weapon_type)
        }
    }
}
