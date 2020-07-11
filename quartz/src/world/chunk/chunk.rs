use std::collections::HashMap;
use log::{warn, error};
use nbt::{NbtTag, NbtCompound};
use util::{
    UnlocalizedName,
    single_access::{PartialMove, SingleAccessorBox}
};
use crate::block::{self, StateID};
use crate::block::entity::BlockEntity;
use crate::world::location::{BlockPosition, CoordinatePair, ChunkCoordinatePair};

pub struct Chunk {
    block_offset: CoordinatePair,
    sections: Vec<OptionalSection>,
    block_entities: HashMap<BlockPosition, SingleAccessorBox<dyn BlockEntity + Send>>
}

impl Chunk {
    pub fn from_nbt(nbt: &NbtCompound) -> Option<Chunk> {
        let mut sections = Vec::with_capacity(16);
        sections.resize_with(16, Default::default);

        let mut chunk = Chunk {
            block_offset: CoordinatePair::new(0, 0),
            sections,
            block_entities: HashMap::new()
        };

        let level = nbt.get_compound("Level")?;

        chunk.block_offset.x = level.get_int("xPos")?;
        chunk.block_offset.z = level.get_int("zPos")?;

        for section_tag in level.get_list("Sections")?.iter() {
            let section = match section_tag {
                NbtTag::Compound(section) => section,
                _ => {
                    warn!("Ignoring non-compound tag in chunk section palette.");
                    continue;
                }
            };

            let raw_palette = match section.get_list("Palette") {
                Some(palette) => palette,
                None => continue
            };
            let mut palette: Vec<StateID> = vec![0 as StateID; raw_palette.len()];
            let mut index: usize = 0;

            for state_tag in raw_palette.iter() {
                let state = match state_tag {
                    NbtTag::Compound(state) => state,
                    _ => {
                        error!("Cannot parse non-compound state tag: {}", state_tag);
                        return None;
                    }
                };

                let state_name = state.get_string("Name")?;
                let mut state_builder = match block::new_state(&UnlocalizedName::parse(state_name).ok()?) {
                    Some(builder) => builder,
                    None => {
                        error!("Unknown block state encountered: {}", state_name);
                        return None;
                    }
                };

                if let Some(properties) = state.get_compound("Properties") {
                    'properties: for (name, tag) in properties.iter() {
                        let property_value = match tag {
                            NbtTag::StringModUtf8(value) => value,
                            _ => {
                                warn!("Ignoring invalid property value for {}", state_name);
                                continue 'properties;
                            }
                        };

                        if let Err(message) = state_builder.add_property(name, property_value) {
                            warn!("{}", message);
                        }
                    }
                }

                palette[index] = state_builder.build().id();
                index += 1;
            }

            log::info!("{:?}", palette);
        }

        Some(chunk)
    }

    #[inline(always)]
    fn section_index_absolute(&self, pos: &BlockPosition) -> usize {
        ((pos.x - self.block_offset.x) + (pos.z - self.block_offset.z) * 16 + (pos.y as i32 % 16) * 256) as usize
    }

    #[inline]
    pub fn block_id(&self, absolute_position: &BlockPosition) -> StateID {
        match self.sections.get((absolute_position.y as usize) >> 4) {
            Some(section) => section.block_id(self.section_index_absolute(absolute_position)),
            None => 0
        }
    }

    #[inline]
    pub fn chunk_coordinates(&self) -> ChunkCoordinatePair {
        self.block_offset >> 4
    }

    pub fn block_entity_at(&self, absolute_position: &BlockPosition) -> Option<PartialMove<'_, dyn BlockEntity + Send>> {
        self.block_entities.get(absolute_position)?.take()
    }
}

struct OptionalSection(Option<Box<Section>>);

impl OptionalSection {
    fn init(&mut self) {
        self.0 = Some(Box::new(Section::new()))
    }

    fn block_id(&self, index: usize) -> StateID {
        match &self.0 {
            Some(section) => section.block_data.get(index).map(|id| *id).unwrap_or(0),
            None => 0
        }
    }
}

impl Default for OptionalSection {
    fn default() -> Self {
        OptionalSection(None)
    }
}

struct Section {
    block_data: [StateID; 4096],
    lighting: [u8; 2048]
}

impl Section {
    const fn new() -> Self {
        Section {
            block_data: [0 as StateID; 4096],
            lighting: [0_u8; 2048]
        }
    }
}