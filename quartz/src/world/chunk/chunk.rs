use crate::{
    block::{BlockState, StateBuilder},
    world::{
        chunk::{encoder::CompactStateBuffer, ChunkIOError},
        location::{BlockPosition, ChunkCoordinatePair, CoordinatePair},
    },
    Registry,
};
use array_init::array_init;
use log::{error, warn};
use nbt::{NbtCompound, NbtList};
use num_traits::Zero;
use std::{collections::HashMap, str::FromStr, fmt::{self, Debug, Formatter}};
use util::{
    math::fast_log2_64,
    single_access::{AccessGuard, SingleAccessor},
    UnlocalizedName,
};

pub struct Chunk<R: Registry> {
    block_offset: CoordinatePair,
    sections: [Section<R::StateID>; 16],
    block_entities: HashMap<BlockPosition, SingleAccessor<R::BlockEntity>>,
}

impl<R: Registry> Chunk<R> {
    pub fn from_nbt(nbt: &NbtCompound) -> Result<Chunk<R>, ChunkIOError> {
        let mut chunk = Chunk {
            block_offset: CoordinatePair::new(0, 0),
            sections: array_init(|_index| Section::default()),
            block_entities: HashMap::new(),
        };

        // All of the actual data is stored in the inner "Level" tag
        let level: &NbtCompound = nbt.get("Level")?;

        // Chunk coordinates
        chunk.block_offset.x = level.get("xPos")?;
        chunk.block_offset.z = level.get("zPos")?;

        // Iterate over the sections (16x16x16 voxels) that contain block and lighting info
        for section in level
            .get::<_, &NbtList>("Sections")?
            .iter_map::<_, &NbtCompound>()
        {
            let section = section?;

            // The raw palette which contains state information
            let raw_palette = match section.get::<_, &NbtList>("Palette") {
                Ok(palette) => palette,
                Err(_) => continue,
            };

            let mut palette = vec![R::StateID::zero(); raw_palette.len()].into_boxed_slice();
            let mut index: usize = 0;

            // Iterate over the block states in the palette
            for state in raw_palette.iter_map::<_, &NbtCompound>() {
                let state = state?;
                let state_name: &str = state.get("Name")?;

                // Initialize the state builder
                let mut state_builder = match R::BlockState::builder(
                    &UnlocalizedName::from_str(state_name)
                        .map_err(|_| ChunkIOError::InvalidNbtData)?,
                ) {
                    Some(builder) => builder,
                    None => {
                        error!("Unknown block state encountered: {}", state_name);
                        return Err(ChunkIOError::InvalidNbtData);
                    }
                };

                // If the state has property values, add those to the builder
                if let Ok(properties) = state.get::<_, &NbtCompound>("Properties") {
                    for (name, property_value) in properties.iter_map::<_, &String>() {
                        if let Err(message) = state_builder.add_property(name, property_value?) {
                            warn!("{}", message);
                        }
                    }
                }

                palette[index] = state_builder.build().id();
                index += 1;
            }

            // TODO: Make sure that there aren't any bounds checks here, and consider putting data on the stack
            let mut block_states = vec![R::StateID::zero(); 4096].into_boxed_slice();
            let mut state_reader =
                CompactStateBuffer::from(section.get::<_, &[i64]>("BlockStates")?);
            let bits_per_index = Self::bits_for_palette_size(palette.len());

            for state in block_states.iter_mut() {
                *state = state_reader
                    .read_index(bits_per_index)
                    .map(|index| palette.get(index))
                    .flatten()
                    .copied()
                    .ok_or(ChunkIOError::InvalidNbtData)?;
            }

            chunk.sections[section.get::<_, i8>("Y")? as usize] = Section {
                block_states: Some(block_states),
                lighting: section
                    .get::<_, &[i8]>("SkyLight")
                    .map(|lighting| lighting.iter().copied().map(|b| b as u8).collect())
                    .ok(),
            };
        }

        Ok(chunk)
    }

    fn bits_for_palette_size(palette_size: usize) -> usize {
        fast_log2_64(palette_size as u64).max(4) as usize
    }

    #[inline]
    fn section_index_absolute(&self, pos: BlockPosition) -> usize {
        ((pos.x - self.block_offset.x)
            + (pos.z - self.block_offset.z) * 16
            + (pos.y as i32 % 16) * 256) as usize
    }

    #[inline]
    pub fn block_state_at(&self, absolute_position: BlockPosition) -> Option<&'static R::BlockState> {
        match self.sections.get((absolute_position.y as usize) >> 4) {
            Some(section) => R::state_for_id(section.block_id(self.section_index_absolute(absolute_position))),
            None => None,
        }
    }

    #[inline]
    pub fn chunk_coordinates(&self) -> ChunkCoordinatePair {
        self.block_offset >> 4
    }

    pub fn block_entity_at(
        &self,
        absolute_position: BlockPosition,
    ) -> Option<AccessGuard<'_, R::BlockEntity>>
    {
        self.block_entities.get(&absolute_position)?.take()
    }
}

impl<R: Registry> Debug for Chunk<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Chunk@{:?}", self.block_offset >> 5)
    }
}

struct Section<T> {
    block_states: Option<Box<[T]>>,
    lighting: Option<Box<[u8]>>,
}

impl<T> Section<T> {
    const fn new() -> Self {
        Section {
            block_states: None,
            lighting: None,
        }
    }
}

impl<T: Zero + Copy> Section<T> {
    fn init(&mut self) {
        self.block_states = Some(vec![T::zero(); 4096].into_boxed_slice());
        self.lighting = Some(vec![0u8; 2048].into_boxed_slice());
    }

    fn block_id(&self, index: usize) -> T {
        match self.block_states.as_ref() {
            Some(block_states) => block_states.get(index).copied().unwrap_or(T::zero()),
            None => T::zero(),
        }
    }
}

impl<T> Default for Section<T> {
    fn default() -> Self {
        Self::new()
    }
}
