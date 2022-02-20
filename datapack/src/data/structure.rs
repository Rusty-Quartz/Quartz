use std::collections::HashMap;

use qdat::UnlocalizedName;
use quartz_nbt::NbtCompound;
use serde::{Deserialize, Serialize};

/// A structure
///
/// This is stored in NBT files in contrast to the rest of datapacks that are in json
#[derive(Serialize, Deserialize)]
pub struct Structure {
    #[serde(rename = "DataVersion")]
    pub data_version: i32,
    pub size: Vec<i32>,
    pub palette: Option<Vec<StructurePaletteEntry>>,
    pub blocks: Vec<StructureBlock>,
    pub entities: Vec<StructureEntity>,
}

/// An entry in the palette of a structure
#[derive(Serialize, Deserialize)]
pub struct StructurePaletteEntry {
    #[serde(rename = "Name")]
    pub name: UnlocalizedName,
    #[serde(rename = "Properties")]
    #[serde(default)]
    pub properties: HashMap<String, String>,
}

/// A block of a structure
#[derive(Serialize, Deserialize)]
pub struct StructureBlock {
    pub state: i32,
    pub pos: Vec<i32>,
    pub nbt: Option<NbtCompound>,
}

/// An entity in a structure
#[derive(Serialize, Deserialize)]
pub struct StructureEntity {
    pub pos: Vec<f64>,
    #[serde(rename = "blockPos")]
    pub block_pos: Vec<i32>,
    pub nbt: NbtCompound,
}
