use std::collections::{BTreeMap, HashMap};

use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use super::{dimension::StructureSettings, features::SurfaceType};

#[derive(Serialize, Deserialize)]
pub struct NoiseSettings {
    pub sea_level: i32,
    pub disable_mob_generation: bool,
    pub noise_caves_enabled: bool,
    pub noodle_caves_enabled: bool,
    pub ore_veins_enabled: bool,
    pub aquifers_enabled: bool,
    pub legacy_random_source: bool,
    pub default_block: BlockState,
    pub default_fluid: BlockState,
    pub structures: BTreeMap<UnlocalizedName, StructureSettings>,
    pub noise: Noise,
    pub surface_rule: SurfaceRule,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SurfaceRule {
    #[serde(rename = "minecraft:condition")]
    Condition {
        if_true: Box<SurfaceRule>,
        then_run: Box<SurfaceRule>,
    },
    #[serde(rename = "minecraft:block")]
    Block { result_state: BlockState },
    #[serde(rename = "minecraft:vertical_gradient")]
    VerticalGradient {
        random_name: String,
        true_at_and_below: HeightConditionProvider,
        false_at_and_above: HeightConditionProvider,
    },
    #[serde(rename = "minecraft:above_preliminary_surface")]
    AbovePreliminarySurface,
    #[serde(rename = "minecraft:sequence")]
    Sequnce { sequence: Vec<SurfaceRule> },
    #[serde(rename = "minecraft:stone_depth")]
    StoneDepth {
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
        surface_type: SurfaceType,
    },
    #[serde(rename = "minecraft:water")]
    Water {
        offset: i32,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:biome")]
    Biome { biome_is: Vec<UnlocalizedName> },
    #[serde(rename = "minecraft:y_above")]
    YAbove {
        anchor: HeightConditionProvider,
        surface_depth_multiplier: i32,
        add_stone_depth: bool,
    },
    #[serde(rename = "minecraft:not")]
    Not { invert: Box<SurfaceRule> },
    #[serde(rename = "minecraft:noise_threshold")]
    NoiseThreshold {
        noise: NoiseType,
        min_threshold: f64,
        max_threshold: f64,
    },
    #[serde(rename = "minecraft:steep")]
    Steep,
    #[serde(rename = "minecraft:hole")]
    Hole,
    #[serde(rename = "minecraft:bandlands")]
    Bandlands,
    #[serde(rename = "minecraft:temperature")]
    Temperature,
}

#[derive(Serialize, Deserialize)]
pub enum NoiseType {
    #[serde(rename = "minecraft:surface")]
    Surface,
    #[serde(rename = "minecraft:surface_swamp")]
    SurfaceSwamp,
    #[serde(rename = "minecraft:packed_ice")]
    PackedIce,
    #[serde(rename = "minecraft:ice")]
    Ice,
    #[serde(rename = "minecraft:powder_snow")]
    PowderSnow,
    #[serde(rename = "minecraft:calcite")]
    Calcite,
    #[serde(rename = "minecraft:gravel")]
    Gravel,
    #[serde(rename = "minecraft:patch")]
    Patch,
    #[serde(rename = "minecraft:nether_state_selector")]
    NetherStateSelector,
    #[serde(rename = "minecraft:netherrack")]
    Netherrack,
    #[serde(rename = "minecraft:nether_wart")]
    NetherWart,
    #[serde(rename = "minecraft:soul_sand_layer")]
    SoulSandLayer,
    #[serde(rename = "minecraft:gravel_layer")]
    GravelLayer,
}


#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum HeightConditionProvider {
    AboveBottom { above_bottom: i32 },
    Absolute { absolute: i32 },
    BelowTop { below_top: i32 },
}

#[derive(Serialize, Deserialize)]
pub struct Noise {
    pub min_y: i32,
    pub height: i32,
    pub size_horizontal: i32,
    pub size_vertical: i32,
    #[serde(default = "Default::default")]
    pub island_noise_override: bool,
    #[serde(default = "Default::default")]
    pub amplified: bool,
    #[serde(default = "Default::default")]
    pub large_biomes: bool,
    pub sampling: NoiseSampling,
    pub top_slide: NoiseCurve,
    pub bottom_slide: NoiseCurve,
    pub terrain_shaper: TerrainShaper,
}

#[derive(Serialize, Deserialize)]
pub struct TerrainShaper {
    pub offset: TerrainShaperValue,
    pub factor: TerrainShaperValue,
    pub jaggedness: TerrainShaperValue,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum TerrainShaperValue {
    Spline {
        coordinate: String,
        points: Vec<TerrainSplinePoint>,
    },
    Constant(f32),
}

#[derive(Serialize, Deserialize)]
pub struct TerrainSplinePoint {
    pub location: f32,
    pub value: TerrainShaperValue,
    pub derivative: f32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainSpineCoordinate {
    Continents,
    Erosion,
    Ridges,
    Weirdness,
}

#[derive(Serialize, Deserialize)]
pub struct NoiseSampling {
    pub xz_scale: f64,
    pub xz_factor: f64,
    pub y_scale: f64,
    pub y_factor: f64,
}

#[derive(Serialize, Deserialize)]
pub struct NoiseCurve {
    pub target: f32,
    pub size: i32,
    pub offset: i32,
}


#[derive(Serialize, Deserialize)]
pub struct BlockState {
    #[serde(rename = "Name")]
    pub name: UnlocalizedName,
    #[serde(rename = "Properties")]
    #[serde(default = "Default::default")]
    pub properties: HashMap<String, String>,
}
