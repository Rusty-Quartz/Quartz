use crate::{
    base::{BlockEntity, BlockState, StateID},
    block::{states::BlockStateData, BlockStateImpl, StateBuilder},
    network::packet::BlockLights,
    world::{
        chunk::{encoder::CompactStateBuffer, ChunkIoError},
        location::{BlockPosition, Coordinate, CoordinatePair},
    },
    Registry,
};
use log::warn;
use num_traits::Zero;
use quartz_nbt::{NbtCompound, NbtList, NbtReprError};
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
    section_mapping: [i8; MAX_SECTION_COUNT],
    sections: Vec<Section>,
    block_entities: HashMap<u64, SingleAccessor<BlockEntity>, NumHasher>,
    // We store the heightmaps just as nbt, this could be improved in the future to reduce memory usage
    heightmaps: NbtCompound,
    biomes: [i32; 1024],
}

impl Chunk {
    pub fn from_nbt(nbt: &NbtCompound) -> Result<Chunk, ChunkIoError> {
        let mut chunk = Chunk {
            block_offset: CoordinatePair::new(0, 0),
            section_mapping: [-1i8; MAX_SECTION_COUNT],
            sections: Vec::new(),
            block_entities: HashMap::with_hasher(NumHasher),
            heightmaps: NbtCompound::new(),
            biomes: [0; 1024],
        };

        // All of the actual data is stored in the inner "Level" tag
        let level: &NbtCompound = nbt.get("Level")?;

        // Chunk coordinates
        chunk.block_offset.x = level.get::<_, i32>("xPos")? * 16;
        chunk.block_offset.z = level.get::<_, i32>("zPos")? * 16;

        chunk.heightmaps = level.get::<_, &NbtCompound>("Heightmaps")?.clone();

        match level.get::<_, &Vec<i32>>("Biomes") {
            Ok(biomes) =>
            // Since we store biomes in a fixed sized array I don't know how else to do this
            // biomes will never be larger than 1024 so indexing chunk.biomes won't panic
                for i in 0 .. biomes.len() {
                    chunk.biomes[i] = biomes[i]
                },
            Err(_) => {}
        };

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
                    .map_err(|error| ChunkIoError::InvalidNbtData(error.to_owned()))?;
                let mut state_builder = match BlockState::builder(&block_name) {
                    Some(builder) => builder,
                    None => {
                        return Err(ChunkIoError::InvalidNbtData(format!(
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
                    .ok_or(ChunkIoError::InvalidNbtData(
                        "Failed to map state index to palette.".to_owned(),
                    ))?;
                unsafe {
                    ptr::write(block_states.add(offset), state);
                }
            }

            // Safety: same as for block_states
            let sky_light_raw = unsafe { ptr::addr_of_mut!((*raw_section).sky_light) as *mut u8 };

            match section.get::<_, &[i8]>("SkyLight") {
                // If the lighting section exists, verify its length and copy it
                Ok(lighting) => {
                    if lighting.len() != 2048 {
                        return Err(ChunkIoError::InvalidNbtData(format!(
                            "Invalid SkyLight field length: {}",
                            lighting.len()
                        )));
                    }

                    // Safety: i8 and u8 have equivalent representations, and the length was asserted above
                    unsafe {
                        let src = lighting.as_ptr() as *const u8;
                        ptr::copy_nonoverlapping(src, sky_light_raw, 2048);
                    }
                }
                // If the tag was missing, when write all zeroes
                Err(NbtReprError::Structure(_)) => unsafe {
                    ptr::write_bytes(sky_light_raw, 0, 2048);
                },
                // Any other error is a hard error
                Err(e) => return Err(e.into()),
            }

            // Safety: same as for block_states
            let block_light_raw =
                unsafe { ptr::addr_of_mut!((*raw_section).block_light) as *mut u8 };

            match section.get::<_, &[i8]>("BlockLight") {
                // If the lighting section exists, verify its length and copy it
                Ok(lighting) => {
                    if lighting.len() != 2048 {
                        return Err(ChunkIoError::InvalidNbtData(format!(
                            "Invalid SkyLight field length: {}",
                            lighting.len()
                        )));
                    }

                    // Safety: i8 and u8 have equivalent representations, and the length was asserted above
                    unsafe {
                        let src = lighting.as_ptr() as *const u8;
                        ptr::copy_nonoverlapping(src, block_light_raw, 2048);
                    }
                }
                // If the tag was missing, when write all zeroes
                Err(NbtReprError::Structure(_)) => unsafe {
                    ptr::write_bytes(block_light_raw, 0, 2048);
                },
                // Any other error is a hard error
                Err(e) => return Err(e.into()),
            }

            chunk.section_mapping[section.get::<_, i8>("Y")? as usize] = chunk.sections.len() as i8;

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
        let index = self.section_mapping[y];
        index == -1 || self.sections[index as usize].is_empty()
    }

    pub fn get_heightmaps(&self) -> NbtCompound {
        self.heightmaps.clone()
    }

    /// Loops over the sections and produces a bitmask where a bit set means the section is not empty
    ///
    /// The lowest section is the least significant bit
    // the bitmask is most likely a vec in order to support variable world heights
    // TODO: properly support chunks with more than 64 sections
    pub fn get_bitmask(&self) -> Vec<i64> {
        let mut mask = 0;
        for index in self.section_mapping.iter().rev() {
            mask = mask << 1;
            if *index != -1 {
                let section = &self.sections[*index as usize];
                mask |= !section.is_empty() as i64;
            }
        }
        vec![mask]
    }

    pub fn get_biomes(&self) -> Vec<i32> {
        self.biomes.to_vec()
    }

    /// Gets the blocklights and blocklight bitmask for the chunk
    pub fn get_blocklights(&self) -> (Vec<i64>, Vec<i64>, Vec<BlockLights>) {
        let mut mask = 0;
        let mut empty_mask = 0;
        let mut blocklights = Vec::new();
        for index in self.section_mapping.iter().rev() {
            mask = mask << 1;
            empty_mask = empty_mask << 1;
            if *index != -1 {
                let section = &self.sections[*index as usize];
                if section.has_block_light() {
                    mask |= 1;
                    blocklights.push(section.get_block_light());
                } else {
                    empty_mask |= 1;
                }
            } else {
                empty_mask |= 1;
            }
        }
        // shift off the lowest bit because mojang has us send a mask entry for one below the world
        // TODO: add bitmask support to negative y-values
        // reverse because masks and sections are in oposite orders
        mask = mask << 1;
        empty_mask = empty_mask << 1;
        empty_mask |= 1;
        blocklights.reverse();
        (vec![mask], vec![empty_mask], blocklights)
    }

    /// Gets the skylights and skylight bitmask for the chunk
    pub fn get_skylights(&self) -> (Vec<i64>, Vec<i64>, Vec<BlockLights>) {
        let mut mask = 0;
        let mut empty_mask = 0;
        let mut skylights = Vec::new();
        for index in self.section_mapping.iter().rev() {
            mask = mask << 1;
            empty_mask = empty_mask << 1;
            if *index != -1 {
                let section = &self.sections[*index as usize];
                if section.has_sky_light() {
                    mask |= 1;
                    skylights.push(section.get_sky_light());
                } else {
                    empty_mask |= 1;
                }
            } else {
                empty_mask |= 1;
            }
        }
        // shift off the lowest bit because mojang has us send a mask entry for one below the world
        // TODO: add bitmask support to negative y-values
        mask = mask << 1;
        empty_mask = empty_mask << 1;
        empty_mask |= 1;
        // reverse because masks and sections are in oposite orders
        skylights.reverse();
        (vec![mask], vec![empty_mask], skylights)
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

struct Section {
    block_states: [StateID; 4096],
    sky_light: [u8; 2048],
    block_light: [u8; 2048],
}

impl Section {
    const fn new() -> Self {
        Section {
            block_states: [0; 4096],
            sky_light: [0; 2048],
            block_light: [0; 2048],
        }
    }

    pub fn is_empty(&self) -> bool {
        self.block_states == [0; 4096]
    }

    fn block_id(&self, index: usize) -> StateID {
        self.block_states
            .get(index)
            .copied()
            .unwrap_or(StateID::zero())
    }

    fn get_client_section(&self) -> ClientSection {
        let (bits_per_block, palette) = self.gen_palette();

        let mut data = Vec::new();
        let mut num_blocks = 0;
        let mut current_long = 0_i64;
        let mut space_in_long = 64_u8;

        for state in self.block_states.iter().rev() {
            let state = *state;
            if state != BlockStateData::Air.id()
                && state != BlockStateData::CaveAir.id()
                && state != BlockStateData::VoidAir.id()
            {
                num_blocks += 1;
            }

            let pal_index = if let Some(pal) = &palette {
                // the state has to be in the palette
                let pal_index = pal.iter().position(|u| *u == state as i32).unwrap();
                pal_index as i64
            } else {
                state as i64
            };

            current_long |= pal_index as i64;

            space_in_long -= bits_per_block;

            if space_in_long < bits_per_block {
                data.push(current_long);
                current_long = 0;
                space_in_long = 64;
            } else {
                current_long = current_long << bits_per_block;
            }
        }

        data.reverse();

        ClientSection {
            block_count: num_blocks,
            palette,
            bits_per_block,
            data: data.into_boxed_slice(),
        }
    }

    fn gen_palette(&self) -> (u8, Option<Box<[i32]>>) {
        let mut found_ids = Vec::new();
        found_ids.push(0);
        for state in self.block_states {
            if !found_ids.contains(&(state as i32)) {
                found_ids.push(state as i32)
            }
        }
        let bits_per_block = crate::util::math::fast_log2_64(found_ids.len() as u64) as u8;

        if bits_per_block < 4 {
            (4, Some(found_ids.into_boxed_slice()))
        } else if bits_per_block > 8 {
            (BITS_PER_BLOCK, None)
        } else {
            (bits_per_block, Some(found_ids.into_boxed_slice()))
        }
    }

    fn has_block_light(&self) -> bool {
        self.block_light != [0; 2048]
    }

    fn get_block_light(&self) -> BlockLights {
        BlockLights {
            values: Box::new(self.block_light.clone()),
        }
    }

    fn has_sky_light(&self) -> bool {
        self.sky_light != [0; 2048]
    }

    fn get_sky_light(&self) -> BlockLights {
        BlockLights {
            values: Box::new(self.sky_light.clone()),
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
