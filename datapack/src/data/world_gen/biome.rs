use std::collections::HashMap;

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::tags::IdsOrTag;

#[derive(Serialize, Deserialize)]
pub struct Biome {
    pub category: BiomeCategory,
    pub precipitation: Precipitation,
    pub temperature: f32,
    pub tempearture_modifier: Option<TemperatureModifier>,
    pub downfall: f32,
    pub creature_spawn_probability: Option<f32>,
    pub effects: BiomeEffects,
    pub carvers: BiomeCarvers,
    pub features: Vec<IdsOrTag>,
    pub spawners: HashMap<MobCategory, Vec<MobSpawners>>,
    // This is literally never used by vanilla
    pub spawn_costs: HashMap<String, SpawnCosts>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Precipitation {
    None,
    Rain,
    Snow,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BiomeCategory {
    None,
    Taiga,
    ExtremeHills,
    Jungle,
    Mesa,
    Plains,
    Savanna,
    Icy,
    TheEnd,
    Beach,
    Forest,
    Ocean,
    Desert,
    River,
    Swamp,
    Mushroom,
    Nether,
    Underground,
    Mountain,
}

#[derive(Serialize, Deserialize)]
pub enum TemperatureModifier {
    None,
    Frozen,
}

#[derive(Serialize, Deserialize)]
pub struct BiomeEffects {
    pub fog_color: Option<i32>,
    pub foliage_color: Option<i32>,
    pub grass_color: Option<i32>,
    pub sky_color: Option<i32>,
    pub water_color: Option<i32>,
    pub particle: Option<BiomeParticle>,
    pub additions_sound: Option<AdditionalSound>,
    pub ambient_sound: Option<String>,
    pub mood_sound: Option<MoodSound>,
    pub music: Option<BiomeMusic>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrassColorModifier {
    None,
    DarkForest,
    Swamp,
}

#[derive(Serialize, Deserialize)]
pub struct BiomeParticle {
    pub probability: f32,
    pub options: BiomeParticleOptions,
}

#[derive(Serialize, Deserialize)]
pub struct BiomeParticleOptions {
    r#type: UnlocalizedName,
}
// Idk if this is still valid
// #[derive(Serialize, Deserialize)]
// #[serde(tag = "type")]
// pub enum BiomeParticleOptions {
//     #[serde(rename = "minecraft:block")]
//     Block {
//         #[serde(rename = "Name")]
//         name: UnlocalizedName,
//         #[serde(rename = "Properties")]
//         properties: HashMap<String, String>,
//     },
//     #[serde(rename = "minecraft:falling_dust")]
//     FallingDust {
//         #[serde(rename = "Name")]
//         name: UnlocalizedName,
//         #[serde(rename = "Properties")]
//         properties: HashMap<String, String>,
//     },
//     #[serde(rename = "minecraft:dust")]
//     Dust { r: f32, g: f32, b: f32, scale: f32 },
//     #[serde(rename = "minecraft:item")]
//     Item {
//         id: UnlocalizedName,
//         #[serde(rename = "Count")]
//         count: i32,
//         // TO DO: this is snbt
//         tag: String,
//     },
// }

#[derive(Serialize, Deserialize)]
pub struct MoodSound {
    pub sound: String,
    pub tick_delay: i32,
    pub block_search_extent: i32,
    pub offset: f64,
}

#[derive(Serialize, Deserialize)]
pub struct AdditionalSound {
    pub sound: String,
    pub tick_chance: f64,
}

#[derive(Serialize, Deserialize)]
pub struct BiomeMusic {
    pub sound: String,
    pub min_delay: i32,
    pub max_delay: i32,
    pub replace_current_music: bool,
}

#[derive(Serialize, Deserialize)]
pub struct MobSpawners {
    pub r#type: UnlocalizedName,
    pub weight: i32,
    #[serde(rename = "minCount")]
    pub min_count: i32,
    #[serde(rename = "maxCount")]
    pub max_count: i32,
}

#[derive(Serialize, Deserialize)]
pub struct SpawnCosts {
    pub energy_budget: f64,
    pub charge: f64,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MobCategory {
    Monster,
    Creature,
    Ambient,
    WaterCreature,
    UndergroundWaterCreature,
    WaterAmbient,
    Axolotls,
    Misc,
}

#[derive(Serialize, Deserialize)]
pub struct BiomeCarvers {
    pub air: Option<BiomeCarver>,
    pub liquid: Option<BiomeCarver>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum BiomeCarver {
    Singleton(UnlocalizedName),
    List(#[serde(default)] Vec<UnlocalizedName>),
}
