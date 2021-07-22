use crate::{
    base::{BlockState, StateID},
    block::{states::BlockStateData, BlockStateImpl, StateBuilder},
    network::packet::BlockLights,
    world::{
        chunk::encoder::CompactStateBuffer,
        location::{BlockPosition, Coordinate, CoordinatePair},
    },
    Registry,
};
use quartz_nbt::{NbtCompound, NbtList};
use quartz_util::{
    math::fast_ceil_log2_64,
    UnlocalizedName,
};
use serde::{
    de::{self, DeserializeSeed, Visitor},
    Deserialize,
    Serialize,
};
use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    ptr,
    str::FromStr,
};

pub const MAX_SECTION_COUNT: usize = 32;

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

#[derive(Serialize, Deserialize)]
struct RawPaletteEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Properties")]
    #[serde(default = "HashMap::new")]
    properties: HashMap<String, String>,
}

pub struct Chunk {
    block_offset: CoordinatePair,
    section_store: SectionStore,
    // We store the heightmaps just as nbt, this could be improved in the future to reduce memory usage
    heightmaps: NbtCompound,
    biomes: Box<[i32]>,
}

impl Into<Chunk> for RawChunk {
    fn into(self) -> Chunk {
        let level = self.level;
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
    pub fn coordinates(&self) -> Coordinate {
        Coordinate::Block(self.block_offset)
    }

    fn bits_for_palette_size(palette_size: usize) -> u8 {
        match fast_ceil_log2_64(palette_size as u64).max(4) {
            9 ..= 16 => BITS_PER_BLOCK,
            b @ _ => b as u8,
        }
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
            .section_store
            .section_mapping
            .get((absolute_position.y as usize) >> 4)
            .map(|&index| &self.section_store.sections[index as usize])
        {
            Some(section) => Registry::state_for_id(
                section.block_id(self.section_index_absolute(absolute_position))?,
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
        match self
            .section_store
            .section_mapping
            .get((absolute_position.y as usize) >> 4)
        {
            Some(&section_index) => {
                let last_state =
                    self.section_store.sections[section_index as usize].set_state(index, state)?;
                Registry::state_for_id(last_state)
            }
            None => None,
        }
    }

    pub fn get_client_sections(&self) -> Vec<ClientSection> {
        self.section_store
            .sections
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
        let index = self.section_store.section_mapping[y];
        index == -1 || self.section_store.sections[index as usize].is_empty()
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
        for index in self.section_store.section_mapping.iter().skip(1).rev() {
            mask = mask << 1;
            if *index != -1 {
                let section = &self.section_store.sections[*index as usize];
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
        let mut blocklights = Vec::new();
        for index in self.section_store.section_mapping.iter().rev() {
            mask = mask << 1;
            if *index != -1 {
                let section = &self.section_store.sections[*index as usize];
                if section.has_block_light() {
                    mask |= 1;
                    blocklights.push(section.get_block_light());
                }
            }
        }
        // TODO: add bitmask support to negative y-values
        let mut empty_mask = !mask;
        empty_mask |= 1;
        blocklights.reverse();
        (vec![mask], vec![empty_mask], blocklights)
    }

    /// Gets the skylights and skylight bitmask for the chunk
    pub fn get_skylights(&self) -> (Vec<i64>, Vec<i64>, Vec<BlockLights>) {
        let mut mask = 0;
        let mut skylights = Vec::new();
        for index in self.section_store.section_mapping.iter().rev() {
            mask = mask << 1;
            if *index != -1 {
                let section = &self.section_store.sections[*index as usize];
                if section.has_sky_light() {
                    mask |= 1;
                    skylights.push(section.get_sky_light());
                }
            }
        }
        // TODO: add bitmask support to negative y-values
        let mut empty_mask = !mask;
        empty_mask |= 1;
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

struct SectionStore {
    section_mapping: [i8; MAX_SECTION_COUNT],
    sections: Vec<Section>,
}

struct SectionStoreVisitor;

struct DeserializeOne {
    section_ptr: *mut Section,
    y: i8,
}

impl<'de> Visitor<'de> for SectionStoreVisitor {
    type Value = SectionStore;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Array of section")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: serde::de::SeqAccess<'de> {
        let mut store = SectionStore {
            section_mapping: [-1; MAX_SECTION_COUNT],
            sections: Vec::with_capacity(seq.size_hint().unwrap_or(0)),
        };

        loop {
            store.sections.reserve(1);
            let seed = DeserializeOne {
                section_ptr: unsafe { store.sections.as_mut_ptr().add(store.sections.len()) },
                y: 127,
            };

            match seq.next_element_seed(seed)? {
                Some((y, valid)) => {
                    if y == 127 {
                        return Err(de::Error::custom("invalid Y value on section"));
                    }

                    if valid {
                        store.section_mapping[(y + 1) as usize] = store.sections.len() as i8;
                        unsafe {
                            store.sections.set_len(store.sections.len() + 1);
                        }
                    }
                }
                // If the section is invalid we just break and hope we got the rest
                _ => break,
            }
        }

        Ok(store)
    }
}

impl<'de> DeserializeSeed<'de> for DeserializeOne {
    type Value = (i8, bool);

    fn deserialize<D>(mut self, deserializer: D) -> Result<Self::Value, D::Error>
    where D: serde::Deserializer<'de> {
        struct SectionVisitor<'s>(&'s mut DeserializeOne);

        impl<'de, 's> Visitor<'de> for SectionVisitor<'s> {
            type Value = bool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a section")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where A: serde::de::MapAccess<'de> {
                let mut bit_flags = 0_u8;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "Y" => self.0.y = map.next_value()?,
                        "BlockLight" => {
                            // TODO: make this read &[u8]
                            let bytes: Box<[u8]> = map.next_value()?;
                            if bytes.len() != 2048 {
                                return Err(de::Error::custom("Blocklight length not 2048"));
                            }
                            unsafe {
                                let dest_bytes =
                                    ptr::addr_of_mut!((*self.0.section_ptr).block_light) as *mut u8;
                                ptr::copy_nonoverlapping(bytes.as_ptr(), dest_bytes, 2048);
                            };
                            bit_flags |= 1;
                        }
                        "SkyLight" => {
                            // TODO: make this read &[u8]
                            let bytes: Box<[u8]> = map.next_value()?;
                            if bytes.len() != 2048 {
                                return Err(de::Error::custom("Skylight length not 2048"));
                            }
                            unsafe {
                                let dest_bytes =
                                    ptr::addr_of_mut!((*self.0.section_ptr).sky_light) as *mut u8;
                                ptr::copy_nonoverlapping(bytes.as_ptr(), dest_bytes, 2048);
                            };
                            bit_flags |= 0b10;
                        }
                        "BlockStates" => {
                            let data: Vec<i64> = map.next_value()?;


                            unsafe {
                                ptr::addr_of_mut!((*self.0.section_ptr).block_states).write(data)
                            }
                            bit_flags |= 0b100;
                        }
                        "Palette" => {
                            let raw_pal: Vec<RawPaletteEntry> = map.next_value()?;
                            let bits_per_block = Chunk::bits_for_palette_size(raw_pal.len());

                            let mut palette = Vec::new();
                            for palette_entry in raw_pal.into_iter() {
                                let block_name =
                                    UnlocalizedName::from_str(&palette_entry.name).unwrap();
                                let mut state = BlockState::builder(&block_name).unwrap();

                                for (key, value) in &palette_entry.properties {
                                    state.add_property(key, value).unwrap();
                                }
                                palette.push(state.build().id());
                            }

                            unsafe {
                                ptr::addr_of_mut!((*self.0.section_ptr).palette).write(palette);
                                ptr::addr_of_mut!((*self.0.section_ptr).bits_per_block)
                                    .write(bits_per_block);
                            }
                            bit_flags |= 0b1000;
                        }
                        _ => return Err(de::Error::custom("Unexpected key on section")),
                    }
                }
                if bit_flags != 0b1111 {
                    if bit_flags & 1 == 0 {
                        unsafe {
                            let dest_bytes =
                                ptr::addr_of_mut!((*self.0.section_ptr).block_light) as *mut u8;
                            ptr::copy_nonoverlapping(&[0_u8; 2048] as *const u8, dest_bytes, 2048);
                        };
                        bit_flags |= 1;
                    }
                    if bit_flags & 0b10 == 0 {
                        unsafe {
                            let dest_bytes =
                                ptr::addr_of_mut!((*self.0.section_ptr).sky_light) as *mut u8;
                            ptr::copy_nonoverlapping(&[0_u8; 2048] as *const u8, dest_bytes, 2048);
                        };
                        bit_flags |= 0b10;
                    }
                }
                Ok(bit_flags == 0b1111)
            }
        }

        let check = deserializer.deserialize_map(SectionVisitor(&mut self))?;
        Ok((self.y, check))
    }
}

impl<'de> Deserialize<'de> for SectionStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_seq(SectionStoreVisitor)
    }
}


pub struct Section {
    palette: Vec<StateID>,
    bits_per_block: u8,
    block_states: Vec<i64>,
    sky_light: [u8; 2048],
    block_light: [u8; 2048],
}

impl Section {
    fn is_empty(&self) -> bool {
        &self.palette == &[0]
    }

    fn block_id(&self, index: usize) -> Option<StateID> {
        let mut buffer =
            CompactStateBuffer::<&_>::new(&self.block_states, self.bits_per_block as usize);
        buffer.skip(index);
        let palette_index = buffer.read_index()?;
        self.palette.get(palette_index).copied()
    }

    /// Sets the block state at index to state
    fn set_state(&mut self, index: usize, state: StateID) -> Option<StateID> {
        // TODO: handle removing the old state from the palette if we no longer need it
        if !self.palette.contains(&state) {
            self.add_state_to_palette(state)
        }

        let mut buffer =
            CompactStateBuffer::<&mut _>::new(&mut self.block_states, self.bits_per_block as usize);
        buffer.skip(index);
        let last_index = buffer.peak_index()?;
        buffer.write_index(self.palette.iter().position(|s| s == &state)?);
        self.palette.get(last_index).cloned()
    }

    /// Pushes a state onto the palette, updating `block_states` if `bits_per_block` would change
    fn add_state_to_palette(&mut self, state: StateID) {
        self.palette.push(state);
        if Chunk::bits_for_palette_size(self.palette.len() + 1) != self.bits_per_block {
            // The indexes of states already there won't have changed so we can just pass in our palette
            // clones because we can't pass in both an &mut and a & to self
            self.update_bits_per_block(&self.palette.clone())
        }
    }

    /// Removes the state at `index` from the palette
    ///
    /// If this would change `bits_per_block` or adjust the indecies of other elements in the palette we regenerate `block_states`
    fn remove_state_from_palette(&mut self, index: usize) -> Result<(), String> {
        if index >= self.palette.len() {
            Err(format!(
                "Index {} is out of bounds for palette of length {}",
                index,
                self.palette.len()
            ))
        } else if index == self.palette.len() - 1
            && Chunk::bits_for_palette_size(self.palette.len() - 1) == self.bits_per_block
        {
            // If we are not adjusting the indexes of other elements
            // and we aren't changing the bits_per_block we can just remove the element
            self.palette.remove(index);
            Ok(())
        } else {
            let mut new_palette = self.palette.clone();
            new_palette.remove(index);
            self.update_bits_per_block(&new_palette);
            self.palette = new_palette;
            Ok(())
        }
    }

    /// Replaces the current palette with `new_palette` and regenerates `block_states` to match
    fn replace_palette(&mut self, new_palette: Vec<StateID>) {
        // Since we have no guarentee that palette items are in the same order
        // we need to update our blockstate data before we update the palette
        self.update_bits_per_block(&new_palette);
        self.palette = new_palette;
    }

    /// Updates the data stored in block_states to match the new palette
    ///
    /// Sets block_states and bits_per_block to their new values
    fn update_bits_per_block(&mut self, new_palette: &Vec<StateID>) {
        let bits_per_block = Chunk::bits_for_palette_size(new_palette.len());
        let mut new_data = Vec::new();
        let mut current_long = 0;
        let mut space_in_long = 64;
        let mut buf =
            CompactStateBuffer::<&_>::new(&self.block_states, self.bits_per_block as usize);

        while let Some(entry) = buf.read_index() {
            let state = self.palette.get(entry).unwrap();
            space_in_long -= bits_per_block;
            if bits_per_block > 8 {
                current_long |= *state as i64;
            } else {
                // unwrap is safe as long as no existing states have been removed from the palette
                current_long |= new_palette.iter().position(|s| *s == *state).unwrap() as i64;
            }

            if space_in_long < bits_per_block {
                new_data.push(current_long);
                current_long = 0;
                space_in_long = 64;
            } else {
                current_long = current_long << bits_per_block;
            }
        }

        self.block_states = new_data;
        self.bits_per_block = bits_per_block;
    }

    /// Gets the number of non-air blocks in the section
    // TODO: make this more efficient
    // I feel like with clever iter tricks or something we could make this not just be a loop over 4096 values
    fn get_block_count(&self) -> i16 {
        let mut count = 0;
        let mut buffer =
            CompactStateBuffer::<&_>::new(&self.block_states, self.bits_per_block as usize);
        while let Some(index) = buffer.read_index() {
            let state = if self.bits_per_block != BITS_PER_BLOCK {
                *self.palette.get(index).unwrap()
            } else {
                index as u16
            };
            if state != BlockStateData::Air.id()
                && state != BlockStateData::CaveAir.id()
                && state != BlockStateData::VoidAir.id()
            {
                count += 1;
            }
        }
        count
    }

    fn get_client_section(&self) -> ClientSection {
        ClientSection {
            block_count: self.get_block_count(),
            palette: if self.bits_per_block == BITS_PER_BLOCK {
                None
            } else {
                Some(
                    self.palette
                        .iter()
                        .map(|s| *s as i32)
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                )
            },
            bits_per_block: self.bits_per_block,
            data: self.block_states.clone().into_boxed_slice(),
        }
    }

    fn has_block_light(&self) -> bool {
        self.block_light.iter().any(|&s| s != 0)
    }

    fn get_block_light(&self) -> BlockLights {
        BlockLights {
            values: Box::new(self.block_light.clone()),
        }
    }

    fn has_sky_light(&self) -> bool {
        self.sky_light.iter().any(|&s| s != 0)
    }

    fn get_sky_light(&self) -> BlockLights {
        BlockLights {
            values: Box::new(self.sky_light.clone()),
        }
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
