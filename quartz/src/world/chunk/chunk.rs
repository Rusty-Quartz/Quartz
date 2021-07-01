use crate::{
    block::{BlockStateImpl, StateBuilder},
    world::{
        chunk::{encoder::CompactStateBuffer, ChunkIOError},
        location::{BlockPosition, Coordinate, CoordinatePair},
    },
    base::{BlockState, StateID, BlockEntity},
    Registry
};
use log::warn;
use num_traits::Zero;
use quartz_nbt::{NbtCompound, NbtList, NbtStructureError, NbtReprError};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    str::FromStr,
};
use quartz_util::{
    math::fast_log2_64,
    single_access::{AccessGuard, SingleAccessor},
    UnlocalizedName,
    hash::NumHasher
};
use std::ptr;

pub const MAX_SECTION_COUNT: usize = 32;

pub struct Chunk {
    block_offset: CoordinatePair,
    section_mapping: [u8; MAX_SECTION_COUNT],
    sections: Vec<Section>,
    block_entities: HashMap<u64, SingleAccessor<BlockEntity>, NumHasher>,
}

impl Chunk {
    pub fn from_nbt(nbt: &NbtCompound) -> Result<Chunk, ChunkIOError> {
        let mut chunk = Chunk {
            block_offset: CoordinatePair::new(0, 0),
            section_mapping: [0u8; MAX_SECTION_COUNT],
            sections: Vec::new(),
            block_entities: HashMap::with_hasher(NumHasher),
        };

        // All of the actual data is stored in the inner "Level" tag
        let level: &NbtCompound = nbt.get("Level")?;

        // Chunk coordinates
        chunk.block_offset.x = level.get("xPos")?;
        chunk.block_offset.z = level.get("zPos")?;

        // Iterate over the sections (16x16x16 voxels) that contain block and lighting info
        for section in level
            .get::<_, &NbtList>("Sections")?
            .iter_map::<&NbtCompound>()
        {
            let section = section?;

            // The raw palette which contains state information
            let raw_palette = match section.get::<_, &NbtList>("Palette") {
                Ok(palette) => palette,
                Err(_) => continue,
            };

            let mut palette = vec![StateID::zero(); raw_palette.len()].into_boxed_slice();
            let mut index: usize = 0;

            // Iterate over the block states in the palette
            for state in raw_palette.iter_map::<&NbtCompound>() {
                let state = state?;
                let state_name: &str = state.get("Name")?;

                // Initialize the state builder
                let block_name = UnlocalizedName::from_str(state_name)
                    .map_err(|error| ChunkIOError::InvalidNbtData(error.to_owned()))?;
                let mut state_builder = match BlockState::builder(&block_name) {
                    Some(builder) => builder,
                    None => {
                        return Err(ChunkIOError::InvalidNbtData(format!("Unknown block state encountered: {}", state_name)));
                    }
                };

                // If the state has property values, add those to the builder
                if let Ok(properties) = state.get::<_, &NbtCompound>("Properties") {
                    for (name, property_value) in properties.iter_map::<&String>() {
                        if let Err(message) = state_builder.add_property(name, property_value?) {
                            warn!("{}", message);
                        }
                    }
                }

                palette[index] = state_builder.build().id();
                index += 1;
            }

            let mut state_reader =
                CompactStateBuffer::from(section.get::<_, &[i64]>("BlockStates")?);
            let bits_per_index = Self::bits_for_palette_size(palette.len());

            // The following construction of the section is done using unsafe code to avoid blowing the
            // cache due to the large size of Section.

            // Reserve an additional section and grab a pointer to it
            chunk.sections.reserve(1);
            // Safety: we just allocated and additional section
            let raw_section = unsafe { chunk.sections.as_mut_ptr().add(chunk.sections.len()) };

            // Safety: we are only dereferencing so that the proc macro can find the appropriate field
            let block_states = unsafe { ptr::addr_of_mut!((*raw_section).block_states) as *mut StateID };

            // Write the states to the array
            for offset in 0..4096 {
                let state = state_reader
                    .read_index(bits_per_index)
                    .map(|index| palette.get(index))
                    .flatten()
                    .copied()
                    .ok_or(ChunkIOError::InvalidNbtData("Failed to map state index to palette.".to_owned()))?;
                unsafe {
                    ptr::write(block_states.add(offset), state);
                }
            }

            // Safety: same as for block_states
            let lighting_raw = unsafe { ptr::addr_of_mut!((*raw_section).lighting) as *mut u8 };

            match section.get::<_, &[i8]>("SkyLight") {
                // If the lighting section exists, verify its length and copy it
                Ok(lighting) => {
                    if lighting.len() != 2048 {
                        return Err(ChunkIOError::InvalidNbtData(format!("Invalid SkyLight field length: {}", lighting.len())));
                    }

                    // Safety: i8 and u8 have equivalent representations, and the length was asserted above
                    unsafe {
                        let src = lighting.as_ptr() as *const u8;
                        ptr::copy_nonoverlapping(src, lighting_raw, 2048);
                    }
                },
                // If the tag was missing, when write all zeroes
                Err(NbtReprError::Structure(NbtStructureError::MissingTag)) => {
                    unsafe {
                        ptr::write_bytes(lighting_raw, 0, 2048);
                    }
                },
                // Any other error is a hard error
                Err(e) => return Err(e.into())
            }

            chunk.section_mapping[section.get::<_, i8>("Y")? as usize] = chunk.sections.len() as u8;

            // Safety: section has been fully initialized
            unsafe {
                chunk.sections.set_len(chunk.sections.len() + 1);
            }
        }

        chunk.sections.shrink_to_fit();
        Ok(chunk)
    }

    pub fn coordinates(&self) -> Coordinate {
        Coordinate::Block(self.block_offset)
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
    pub fn block_state_at(
        &self,
        absolute_position: BlockPosition,
    ) -> Option<&'static BlockState>
    {
        match self.section_mapping.get((absolute_position.y as usize) >> 4).map(|&index| &self.sections[index as usize]) {
            Some(section) =>
                Registry::state_for_id(section.block_id(self.section_index_absolute(absolute_position))),
            None => None,
        }
    }

    pub fn block_entity_at(
        &self,
        absolute_position: BlockPosition,
    ) -> Option<AccessGuard<'_, BlockEntity>>
    {
        self.block_entities.get(&absolute_position.as_u64())?.take()
    }
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Chunk@{:?}", self.block_offset >> 5)
    }
}

struct Section {
    block_states: [StateID; 4096],
    lighting: [u8; 2048],
}

impl Section {
    fn block_id(&self, index: usize) -> StateID {
        self.block_states.get(index).copied().unwrap_or(StateID::zero())
    }
}

