use std::collections::HashMap;
use std::str::FromStr;
use array_init::array_init;
use log::{warn, error};
use nbt::{NbtCompound, NbtList};
use util::{
    UnlocalizedName,
    single_access::{AccessGuard, SingleAccessor}
};
use crate::block::{self, StateID};
use crate::block::entity::TypedBlockEntity;
use crate::world::location::{BlockPosition, CoordinatePair, ChunkCoordinatePair};

pub struct Chunk {
    block_offset: CoordinatePair,
    sections: [OptionalSection; 16],
    block_entities: HashMap<BlockPosition, SingleAccessor<TypedBlockEntity>>
}

impl Chunk {
    pub fn from_nbt(nbt: &NbtCompound) -> Option<Chunk> {
        let mut chunk = Chunk {
            block_offset: CoordinatePair::new(0, 0),
            sections: array_init(|_index| OptionalSection::default()),
            block_entities: HashMap::new()
        };

        // All of the actual data is stored in the inner "Level" tag
        let level: &NbtCompound = nbt.get("Level")?;

        // Chunk coordinates
        chunk.block_offset.x = level.get("xPos")?;
        chunk.block_offset.z = level.get("zPos")?;

        // Iterate over the sections (16x16x16 voxels) that contain block and lighting info
        for section in level.get::<&NbtList>("Sections")?.iter_map::<_, &NbtCompound>() {
            // The raw palette which contains state information
            let raw_palette = match section?.get::<&NbtList>("Palette") {
                Some(palette) => palette,
                None => continue
            };

            let mut palette = vec![0 as StateID; raw_palette.len()].into_boxed_slice();
            let mut index: usize = 0;

            // Iterate over the block states in the palette
            for state in raw_palette.iter_map::<_, &NbtCompound>() {
                let state = state?;
                let state_name: &str = state.get("Name")?;

                // Initialize the state builder
                let mut state_builder = match block::new_state(&UnlocalizedName::from_str(state_name).ok()?) {
                    Some(builder) => builder,
                    None => {
                        error!("Unknown block state encountered: {}", state_name);
                        return None;
                    }
                };

                // If the state has property values, add those to the builder
                if let Some(properties) = state.get::<&NbtCompound>("Properties") {
                    for (name, property_value) in properties.iter_map::<_, &String>() {
                        if let Err(message) = state_builder.add_property(name, property_value?) {
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
    fn section_index_absolute(&self, pos: BlockPosition) -> usize {
        ((pos.x - self.block_offset.x) + (pos.z - self.block_offset.z) * 16 + (pos.y as i32 % 16) * 256) as usize
    }

    #[inline]
    pub fn block_id(&self, absolute_position: BlockPosition) -> StateID {
        match self.sections.get((absolute_position.y as usize) >> 4) {
            Some(section) => section.block_id(self.section_index_absolute(absolute_position)),
            None => 0
        }
    }

    #[inline]
    pub fn chunk_coordinates(&self) -> ChunkCoordinatePair {
        self.block_offset >> 4
    }

    pub fn block_entity_at(&self, absolute_position: BlockPosition) -> Option<AccessGuard<'_, TypedBlockEntity>> {
        self.block_entities.get(&absolute_position)?.take()
    }
}

#[repr(transparent)]
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