use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::tags::TagProvider;

use super::{features::HeightMaps, noise_settings::BlockState};

#[derive(Serialize, Deserialize)]
pub struct ProcessorList {
    processors: Vec<Processor>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "processor_type")]
pub enum Processor {
    #[serde(rename = "minecraft:rule")]
    Rule { rules: Vec<ProcessorRule> },
    #[serde(rename = "minecraft:block_age")]
    BlockAge { mossiness: f32 },
    #[serde(rename = "minecraft:block_ignore")]
    BlockIgnore { blocks: Vec<BlockState> },
    #[serde(rename = "minecraft:gravity")]
    Gravity { heightmap: HeightMaps, offset: i32 },
    #[serde(rename = "minecraft:block_rot")]
    BlockRot { integrity: f32 },
    #[serde(rename = "minecraft:blackstone_replace")]
    BlackstoneReplace,
    #[serde(rename = "minecraft:jigsaw_replacement")]
    JigsawReplacement,
    #[serde(rename = "minecraft:lava_submerged_block")]
    LavaSubmergedBlock,
    #[serde(rename = "minecraft:nop")]
    Nop,
    #[serde(rename = "minecraft:protected_blocks")]
    ProtectedBlocks { value: TagProvider },
}

#[derive(Serialize, Deserialize)]
pub struct ProcessorRule {
    pub position_predicate: Option<PositionPredicate>,
    pub input_predicate: ProcessorPredicate,
    pub location_predicate: ProcessorPredicate,
    pub output_state: BlockState,
    /// Never used in vanilla
    // TODO: this is nbt
    pub output_nbt: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "predicate_type")]
pub enum ProcessorPredicate {
    #[serde(rename = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:axis_aligned_linear_pos")]
    AxisAlignedLinearPos {
        axis: Axis,
        min_chance: f32,
        max_chance: f32,
        min_dist: i32,
        max_dist: i32,
    },
    #[serde(rename = "minecraft:block_match")]
    BlockMatch { block: UnlocalizedName },
    #[serde(rename = "minecraft:blockstate_match")]
    BlockstateMatch { block_state: BlockState },
    #[serde(rename = "minecraft:random_block_match")]
    RandomBlockMatch {
        block: UnlocalizedName,
        probability: f32,
    },
    #[serde(rename = "minecraft:tag_match")]
    TagMatch { tag: UnlocalizedName },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "predicate_type")]
pub enum PositionPredicate {
    #[serde(rename = "minecraft:always_true")]
    AlwaysTrue,
    #[serde(rename = "minecraft:axis_aligned_linear_pos")]
    AxisAlignedLinearPos {
        axis: Axis,
        min_chance: f32,
        max_chance: f32,
        min_dist: i32,
        max_dist: i32,
    },
    #[serde(rename = "minecraft:linear_pos")]
    LinearPos {
        min_chance: f32,
        max_chance: f32,
        min_dist: i32,
        max_dist: i32,
    },
}

#[derive(Serialize, Deserialize)]
pub enum Axis {
    #[serde(rename = "x")]
    X,
    #[serde(rename = "y")]
    Y,
    #[serde(rename = "z")]
    Z,
}
