use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::{structure_set::StructureSetProvider, tags::IdsOrTag};

use super::noise_settings::NoiseSettings;

#[derive(Serialize, Deserialize)]
pub struct Dimension {
    pub r#type: UnlocalizedName,
    pub generator: DimensionGenerator,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DimensionGenerator {
    #[serde(rename = "minecraft:noise")]
    Noise {
        seed: i32,
        settings: DimensionNoiseSettings,
        biome_source: BiomeSourceType,
    },
    #[serde(rename = "minecraft:flat")]
    Flat { settings: SuperflatSettings },
    #[serde(rename = "minecraft:debug")]
    Debug,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum DimensionNoiseSettings {
    Preset(String),
    Settings(Box<NoiseSettings>),
}

#[derive(Serialize, Deserialize)]
pub struct BiomeSource {
    // NOTE: this might not be an uln
    pub biomes: Vec<BiomeSourceBiome>,
    pub r#type: UnlocalizedName,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BiomeSourceType {
    #[serde(rename = "minecraft:vanilla_layered")]
    VanillaLayer {
        large_biomes: bool,
        legacy_biome_init_layer: bool,
    },
    #[serde(rename = "minecraft:multi_noise")]
    MultiNoise { biomes: Vec<BiomeSourceBiome> },
    #[serde(rename = "minecraft:the_end")]
    TheEnd,
    #[serde(rename = "minecraft:fixed")]
    Fixed { biome: String },
    #[serde(rename = "minecraft:checkerboard")]
    CheckerBoard { biomes: IdsOrTag, scale: i32 },
}

#[derive(Serialize, Deserialize)]
pub struct BiomeSourceBiome {
    // Wiki says this can be repeated
    // WHAT THE FUCK DOES THAT MEAN
    pub biome: UnlocalizedName,
    pub parameters: DimensionBiomeParameters,
}

#[derive(Serialize, Deserialize)]
pub struct DimensionBiomeParameters {
    pub erosion: AmountOrRangeArray,
    pub depth: AmountOrRangeArray,
    pub weirdness: AmountOrRangeArray,
    pub offset: AmountOrRangeArray,
    pub temperature: AmountOrRangeArray,
    pub humidity: AmountOrRangeArray,
    pub continentalness: AmountOrRangeArray,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum AmountOrRangeArray {
    Singleton(f32),
    Array([f32; 2]),
}

#[derive(Serialize, Deserialize)]
pub struct BiomeNoise {
    #[serde(rename = "firstOctive")]
    pub first_octive: i32,
    pub amplitudes: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct SuperflatSettings {
    pub layers: Vec<SuperflatLayer>,
    pub biome: String,
    #[serde(default = "Default::default")]
    pub lakes: bool,
    #[serde(default = "Default::default")]
    pub features: bool,
    #[serde(default = "Default::default")]
    pub structure_overrides: Vec<StructureSetProvider>,
}


#[derive(Serialize, Deserialize)]
pub struct SuperflatLayer {
    pub height: i32,
    pub block: UnlocalizedName,
}

#[derive(Serialize, Deserialize)]
pub struct StrongholdSettings {
    pub distance: i32,
    pub count: i32,
    pub spread: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StructureSettings {
    #[serde(rename = "minecraft:random_spread")]
    RandomSpread {
        spacing: i32,
        separation: i32,
        salt: i32,
    },
    #[serde(rename = "minecraft:concentric_rings")]
    ConcentricRings {
        distance: i32,
        spread: i32,
        count: i32,
    },
}
