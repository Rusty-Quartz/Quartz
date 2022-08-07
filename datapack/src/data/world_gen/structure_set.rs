use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

use crate::data::tags::IdsOrTag;

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum StructureSetProvider {
    Reference(UnlocalizedName),
    Inline(StructureSet),
}

#[derive(Serialize, Deserialize)]
pub struct StructureSet {
    pub structures: Vec<WeightedStructure>,
    pub placement: StructurePlacementModifier,
}

#[derive(Serialize, Deserialize)]
pub struct WeightedStructure {
    structure: IdsOrTag,
    weight: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StructurePlacementModifier {
    #[serde(rename = "minecraft:random_spread")]
    RandomSpread {
        spread_type: Option<SpreadType>,
        spacing: u16,
        separation: u16,
        salt: u64,
        locate_offset: Option<[i8; 3]>,
    },
    #[serde(rename = "minecraft:concentric_rings")]
    ConcentricRings {
        distance: u16,
        spread: u16,
        count: u16,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpreadType {
    Linear,
    Triangular,
}
