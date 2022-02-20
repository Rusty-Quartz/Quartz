use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use super::processors::Processor;

#[derive(Serialize, Deserialize)]
pub struct JigsawPool {
    pub name: UnlocalizedName,
    pub fallback: UnlocalizedName,
    pub elements: Vec<JigsawStructure>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum JigsawStructure {
    SingleElement {
        weight: i32,
        element: JigsawElement,
    },
    ElementList {
        weight: i32,
        elements: Vec<WeightedJigsawElement>,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JigsawProjection {
    Rigid,
    TerrainMatching,
}

#[derive(Serialize, Deserialize)]
pub struct WeightedJigsawElement {
    weight: i32,
    #[serde(flatten)]
    element: JigsawElement,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "element_type")]
pub enum JigsawElement {
    #[serde(rename = "minecraft:empty_pool_element")]
    EmptyPoolElement,
    #[serde(rename = "minecraft:list_pool_element")]
    // TODO: I have no idea what the format of this is
    ListPoolElement {},
    #[serde(rename = "minecraft:feature_pool_element")]
    // TODO: see above
    FeaturePoolElement {},
    #[serde(rename = "minecraft:legacy_single_pool_element")]
    LegacySinglePoolElement {
        location: UnlocalizedName,
        projection: JigsawProjection,
        processors: JigsawProcessor,
    },
    #[serde(rename = "minecraft:single_pool_element")]
    SinglePoolElement {
        location: UnlocalizedName,
        processors: JigsawProcessor,
        projection: JigsawProjection,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum JigsawProcessor {
    Singleton(Processor),
    Uln(UnlocalizedName),
    List(Vec<Processor>),
}
