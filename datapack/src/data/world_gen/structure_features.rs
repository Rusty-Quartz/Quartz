use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum StructureFeatures {
    #[serde(rename = "minecraft:bastion_remnant")]
    BastionRemnant {
        start_pool: UnlocalizedName,
        size: i32,
    },
    #[serde(rename = "minecraft:buried_treasure")]
    BuriedTreasure { probability: f32 },
    #[serde(rename = "minecraft:desert_pyramid")]
    DesertPyramid {},
    #[serde(rename = "minecraft:endcity")]
    Endcity {},
    #[serde(rename = "minecraft:fortress")]
    Fortress {},
    #[serde(rename = "minecraft:igloo")]
    Igloo {},
    #[serde(rename = "minecraft:jungle_pyramid")]
    JunglePyramid {},
    #[serde(rename = "minecraft:mansion")]
    Mansion {},
    #[serde(rename = "minecraft:mineshaft")]
    Mineshaft {
        r#type: MineshaftType,
        probability: f32,
    },
    #[serde(rename = "minecraft:monument")]
    Monument {},
    #[serde(rename = "minecraft:nether_fossil")]
    NetherFossil {},
    #[serde(rename = "minecraft:ocean_ruin")]
    OceanRuin {
        biome_temp: BiomeTemperature,
        large_probability: f32,
        cluster_probability: f32,
    },
    #[serde(rename = "minecraft:pillager_outpost")]
    PillagerOutpost {
        start_pool: UnlocalizedName,
        size: i32,
    },
    #[serde(rename = "minecraft:ruined_portal")]
    RuinedPortal { portal_type: RuinedPortalType },
    #[serde(rename = "minecraft:shipwreck")]
    Shipwreck {
        #[serde(default = "Default::default")]
        is_beached: bool,
    },
    #[serde(rename = "minecraft:stronghold")]
    Stronghold {},
    #[serde(rename = "minecraft:swamp_hut")]
    SwampHut {},
    #[serde(rename = "minecraft:village")]
    Village {
        start_pool: UnlocalizedName,
        size: i32,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MineshaftType {
    Normal,
    Mesa,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiomeTemperature {
    Warm,
    Cold,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuinedPortalType {
    Standard,
    Desert,
    Jungle,
    Swamp,
    Mountain,
    Ocean,
    Nether,
}
