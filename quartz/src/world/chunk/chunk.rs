use crate::{
    base::{BlockEntity, BlockState, StateID},
    block::{BlockStateImpl, StateBuilder},
    world::{
        chunk::{encoder::CompactStateBuffer, ChunkIOError},
        location::{BlockPosition, Coordinate, CoordinatePair},
    },
    Registry,
};
use log::warn;
use num_traits::Zero;
use quartz_nbt::{NbtCompound, NbtList, NbtReprError, NbtStructureError};
use quartz_util::{
    hash::NumHasher,
    math::fast_log2_64,
    single_access::{AccessGuard, SingleAccessor},
    UnlocalizedName,
};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    ptr,
    str::FromStr,
};

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
        chunk.block_offset.x = level.get::<_, i32>("xPos")? * 16;
        chunk.block_offset.z = level.get::<_, i32>("zPos")? * 16;

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
                        return Err(ChunkIOError::InvalidNbtData(format!(
                            "Unknown block state encountered: {}",
                            state_name
                        )));
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
            let block_states =
                unsafe { ptr::addr_of_mut!((*raw_section).block_states) as *mut StateID };

            // Write the states to the array
            for offset in 0 .. 4096 {
                let state = state_reader
                    .read_index(bits_per_index)
                    .map(|index| palette.get(index))
                    .flatten()
                    .copied()
                    .ok_or(ChunkIOError::InvalidNbtData(
                        "Failed to map state index to palette.".to_owned(),
                    ))?;
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
                        return Err(ChunkIOError::InvalidNbtData(format!(
                            "Invalid SkyLight field length: {}",
                            lighting.len()
                        )));
                    }

                    // Safety: i8 and u8 have equivalent representations, and the length was asserted above
                    unsafe {
                        let src = lighting.as_ptr() as *const u8;
                        ptr::copy_nonoverlapping(src, lighting_raw, 2048);
                    }
                }
                // If the tag was missing, when write all zeroes
                Err(NbtReprError::Structure(NbtStructureError::MissingTag)) => unsafe {
                    ptr::write_bytes(lighting_raw, 0, 2048);
                },
                // Any other error is a hard error
                Err(e) => return Err(e.into()),
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
    pub fn block_state_at(&self, absolute_position: BlockPosition) -> Option<&'static BlockState> {
        match self
            .section_mapping
            .get((absolute_position.y as usize) >> 4)
            .map(|&index| &self.sections[index as usize])
        {
            Some(section) => Registry::state_for_id(
                section.block_id(self.section_index_absolute(absolute_position)),
            ),
            None => None,
        }
    }

    pub fn block_entity_at(
        &self,
        absolute_position: BlockPosition,
    ) -> Option<AccessGuard<'_, BlockEntity>> {
        self.block_entities.get(&absolute_position.as_u64())?.take()
    }

    pub fn get_client_sections(&self) -> Vec<ClientSection> {
        self.sections
            .iter()
            .filter_map(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.get_client_section())
                }
            })
            .collect()
    }

    pub fn is_section_empty(&self, y: usize) -> bool {
        self.sections[self.section_mapping[y] as usize].is_empty()
    }
}

impl Debug for Chunk {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Chunk@{:?}", self.block_offset)
    }
}

// TODO: calculate this either on start or in codegen based on how many blocks we have
/// Hardcoded value of how many bits to use per block in the palette
pub const BITS_PER_BLOCK: u8 = 15;

/// The length of the data section of a ClientSection
pub const SECTION_DATA_LENGTH: i32 = (16 * 16 * 16) * BITS_PER_BLOCK as i32 / 64;

/// The size of a ClientSection in bytes
// The final 2 added is the size of the varint repr of SECTION_DATA_LENGTH
// TODO: maybe add a const fn varint_size so we know the size of SECTION_DATA_LENGTH
pub const CLIENT_SECTION_SIZE: i32 = 2 + 1 + (8 * SECTION_DATA_LENGTH) + 2;

struct Section {
    block_states: [StateID; 4096],
    lighting: [u8; 2048],
}

impl Section {
    const fn new() -> Self {
        Section {
            block_states: [0; 4096],
            lighting: [0; 2048],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.block_states.eq(&[0; 4096])
    }

    fn block_id(&self, index: usize) -> StateID {
        self.block_states
            .get(index)
            .copied()
            .unwrap_or(StateID::zero())
    }
}

impl Section {
    fn get_client_section(&self) -> ClientSection {
        let mut data = [0_i64; SECTION_DATA_LENGTH as usize];
        let mut num_blocks = 0;

        let value_mask = (1 << BITS_PER_BLOCK) - 1_u32;


        for block_num in 0 .. self.block_states.len() as u32 {
            let start_long = (block_num * BITS_PER_BLOCK as u32) / 64;
            let start_offset = (block_num * BITS_PER_BLOCK as u32) % 64;
            let end_long = ((block_num + 1) * BITS_PER_BLOCK as u32 - 1) / 64;

            let mut state = self.block_id(block_num as usize) as u64;

            if state != 0 {
                num_blocks += 1;
            }

            state = state & value_mask as u64;

            let data_entry = data.get_mut(start_long as usize).unwrap();
            *data_entry |= (state << start_offset as usize) as i64;

            if start_long != end_long {
                // debug!("start: {}    end: {}", start_long, end_long);
                *data_entry |= (state >> (64 - start_offset as u64)) as i64;
            }
        }

        ClientSection {
            block_count: num_blocks,
            palette: None,
            bits_per_block: BITS_PER_BLOCK,
            data: Box::new(data),
        }
    }
}

impl Default for Section {
    fn default() -> Self {
        Self::new()
    }
}

/// A chunk section in the format understood by the client
#[derive(Debug)]
pub struct ClientSection {
    pub block_count: i16,
    pub palette: Option<Box<[i32]>>,
    pub bits_per_block: u8,
    pub data: Box<[i64]>,
}
