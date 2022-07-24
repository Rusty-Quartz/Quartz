use crate::{
    base::{BlockState, StateID},
    world::chunk::SectionStore,
    Registry,
};
use qdat::world::{
    lighting::LightBuffer,
    location::{BlockPosition, Coordinate, CoordinatePair},
};
use quartz_nbt::{NbtCompound, NbtList};
use quartz_net::{packet_data::SectionData, BitMask};
use serde::Deserialize;
use std::fmt::{self, Debug, Formatter};

pub struct Chunk {
    block_offset: CoordinatePair,
    section_store: SectionStore,
    // We store the heightmaps just as nbt, this could be improved in the future to reduce memory usage
    heightmaps: NbtCompound,
    biomes: Box<[i32]>,
}

impl From<RawChunk> for Chunk {
    fn from(raw: RawChunk) -> Self {
        let level = raw.level;
        let block_offset = CoordinatePair::new(level.x_pos * 16, level.z_pos * 16);

        Chunk {
            block_offset,
            section_store: level.sections,
            heightmaps: level.heightmaps,
            biomes: level.biomes,
        }
    }
}

impl Chunk {
    pub fn new(
        block_offset: CoordinatePair,
        section_store: SectionStore,
        heightmaps: NbtCompound,
        biomes: Box<[i32]>,
    ) -> Chunk {
        Chunk {
            block_offset,
            section_store,
            heightmaps,
            biomes,
        }
    }

    pub fn coordinates(&self) -> Coordinate {
        Coordinate::Block(self.block_offset)
    }

    #[inline]
    fn section_index_absolute(&self, pos: BlockPosition) -> usize {
        ((pos.x - self.block_offset.x)
            + (pos.z - self.block_offset.z) * 16
            + (pos.y as i32 % 16) * 256) as usize
    }

    #[inline]
    pub fn block_state_at(&self, absolute_position: BlockPosition) -> Option<&'static BlockState> {
        match self.section_store.get(absolute_position.y as i8 >> 4) {
            Some(section) => Registry::state_for_id(
                section.block_state_at(self.section_index_absolute(absolute_position))?,
            ),
            None => None,
        }
    }

    /// Sets the blockstate at the provided position to the new state
    ///
    /// Returns the old state
    pub fn set_block_state_at(
        &mut self,
        absolute_position: BlockPosition,
        state: StateID,
    ) -> Option<&'static BlockState> {
        let index = self.section_index_absolute(absolute_position);
        match self.section_store.get_mut(absolute_position.y as i8 >> 4) {
            Some(section) => Registry::state_for_id(section.set_block_state_at(index, state)?),
            None => None,
        }
    }

    pub fn sections(&self) -> &SectionStore {
        &self.section_store
    }

    pub fn get_heightmaps(&self) -> NbtCompound {
        self.heightmaps.clone()
    }

    pub fn biomes(&self) -> &[i32] {
        &self.biomes
    }

    pub fn gen_client_section_data(&self) -> (BitMask, SectionData) {
        let mut sections = Vec::new();
        let mask = self.section_store.gen_bit_mask(false, |section| {
            let not_empty = !section.is_empty();
            if not_empty {
                sections.push(section.gen_client_section());
            }
            not_empty
        });

        (mask, SectionData {
            sections: sections.into_boxed_slice(),
        })
    }

    /// Gets the blocklights and blocklight bitmask for the chunk
    pub fn gen_block_lights(&self) -> (BitMask, BitMask, Box<[LightBuffer]>) {
        let mut blocklights = Vec::new();
        let mask = self.section_store.gen_bit_mask(true, |section| {
            match section.lighting().block_light() {
                Some(light) => {
                    blocklights.push(light.clone());
                    true
                }
                None => false,
            }
        });

        (mask, mask.as_empty(), blocklights.into_boxed_slice())
    }

    /// Gets the skylights and skylight bitmask for the chunk
    pub fn gen_sky_lights(&self) -> (BitMask, BitMask, Box<[LightBuffer]>) {
        let mut skylights = Vec::new();
        let mask =
            self.section_store
                .gen_bit_mask(true, |section| match section.lighting().sky_light() {
                    Some(light) => {
                        skylights.push(light.clone());
                        true
                    }
                    None => false,
                });

        (mask, mask.as_empty(), skylights.into_boxed_slice())
    }
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Chunk@{:?}", self.block_offset)
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RawChunk {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "Level")]
    level: RawChunkData,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RawClientChunk {
    #[serde(rename = "DataVersion")]
    pub data_version: i32,
    #[serde(rename = "Level")]
    pub level: RawClientChunkData,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RawChunkData {
    #[serde(rename = "Biomes")]
    biomes: Box<[i32]>,
    #[serde(rename = "CarvingMasks")]
    carving_masks: Option<NbtCompound>,
    #[serde(rename = "Heightmaps")]
    heightmaps: NbtCompound,
    #[serde(rename = "LastUpdate")]
    last_update: i64,
    #[serde(rename = "Lights")]
    lights: Option<NbtList>,
    #[serde(rename = "LiquidsToBeTicked")]
    liquids_to_be_ticked: Option<NbtList>,
    #[serde(rename = "LiquidTicks")]
    liquid_ticks: Option<NbtList>,
    #[serde(rename = "InhabitedTime")]
    inhabited_time: i64,
    #[serde(rename = "PostProcessing")]
    post_processing: Option<NbtList>,
    #[serde(rename = "Sections")]
    sections: SectionStore,
    #[serde(rename = "Status")]
    status: String,
    #[serde(rename = "TileEntities")]
    tile_entities: Option<NbtList>,
    #[serde(rename = "TileTicks")]
    tile_ticks: Option<NbtList>,
    #[serde(rename = "ToBeTicked")]
    to_be_ticked: Option<NbtList>,
    #[serde(rename = "Structures")]
    structures: Option<NbtCompound>,
    #[serde(rename = "xPos")]
    x_pos: i32,
    #[serde(rename = "zPos")]
    z_pos: i32,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct RawClientChunkData {
    #[serde(rename = "Biomes")]
    pub biomes: Box<[i32]>,
    #[serde(rename = "Heightmaps")]
    pub heightmaps: NbtCompound,
    #[serde(rename = "Sections")]
    pub sections: SectionStore,
    #[serde(rename = "TileEntities")]
    pub tile_entities: Option<NbtList>,
}
