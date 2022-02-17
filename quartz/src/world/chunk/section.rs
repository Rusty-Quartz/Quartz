use crate::{
    block::{BlockStateImpl, StateBuilder},
    world::chunk::{
        ChunkDecodeError,
        CompactStateBuffer,
        InsertionResult,
        Palette,
        RemovalResult,
        DIRECT_PALETTE_THRESHOLD,
    },
    BlockState,
    StateID,
};
use qdat::{
    block::states::{is_air, AIR},
    world::lighting::{LightBuffer, Lighting},
    UlnStr,
};
use quartz_net::{
    packet_data::{ClientSection, SectionAndLightData, SectionData},
    BitMask,
    PacketBuffer,
    WriteToPacket,
};
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize,
    Serialize,
};
use std::{
    borrow::ToOwned,
    collections::{HashMap, HashSet},
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    num::NonZeroU8,
};

pub const MAX_SECTION_COUNT: usize = 32;

pub struct Section {
    pub y: SectionY,
    is_pal_direct: bool,
    palette: Palette,
    states: CompactStateBuffer,
    lighting: Lighting,
}

impl Section {
    pub fn empty(y: i8) -> Self {
        let palette = Palette::singleton(AIR);
        let states = CompactStateBuffer::new(
            vec![0; CompactStateBuffer::required_capacity(palette.bits_per_block().get())],
            palette.bits_per_block(),
        );

        Section {
            y: y.into(),
            is_pal_direct: false,
            palette,
            states,
            lighting: Lighting::new(),
        }
    }

    fn from_raw(raw: RawSection<'_>) -> Result<Self, ChunkDecodeError> {
        let (palette, states) = if raw.palette.is_none() || raw.block_states.is_none() {
            (Palette::new(), CompactStateBuffer::empty())
        } else {
            let mut palette = Palette::new();

            for palette_entry in raw.palette.unwrap() {
                let mut state = BlockState::builder(palette_entry.name).ok_or_else(|| {
                    ChunkDecodeError::UnknownBlockState(palette_entry.name.to_owned())
                })?;

                for (name, value) in palette_entry.properties {
                    state
                        .add_property(name, value)
                        .map_err(ChunkDecodeError::UnknownStateProperty)?;
                }

                palette.insert(state.build().id());
            }

            let states = CompactStateBuffer::new(
                raw.block_states
                    .unwrap()
                    .into_iter()
                    .map(|x| x as u64)
                    .collect(),
                palette.bits_per_block(),
            );

            (palette, states)
        };

        let mut lighting = Lighting::new();
        if let Some(block_light) = raw.block_light {
            lighting.init_block(block_light)?;
        }
        if let Some(sky_light) = raw.sky_light {
            lighting.init_sky(sky_light)?;
        }

        Ok(Section {
            y: raw.y,
            is_pal_direct: palette.bits_per_block().get() >= DIRECT_PALETTE_THRESHOLD,
            palette,
            states,
            lighting,
        })
    }

    pub fn is_empty(&self) -> bool {
        let quick_check = self.palette.states().all(is_air);

        if quick_check {
            return true;
        }

        self.states
            .iter()
            .map(|entry| self.map_state_entry(entry).unwrap_or(AIR))
            .all(is_air)
    }

    pub fn block_count(&self) -> usize {
        let palette = if !self.is_pal_direct {
            Some(&self.palette)
        } else {
            None
        };

        self.states.block_count(palette)
    }

    /// Returns the block state id at `index`
    ///
    /// Returns `None` if `index` is out of bounds
    pub fn block_state_at(&self, index: usize) -> Option<StateID> {
        if self.is_pal_direct {
            self.states.nth_entry(index).map(|s| s as StateID)
        } else {
            // Unwrap is safe because a palette entry from the buffer has to be in the palette
            let state = self.states.nth_entry(index);
            state.map(|s| self.palette.state_for(s).unwrap())
        }
    }

    /// Sets the block state at `index` to `state`
    ///
    /// Returns the state id that was there
    /// Returns `None` when `index` is out of bounds or if `state` was already at `index`
    pub fn set_block_state_at(&mut self, index: usize, state: StateID) -> Option<StateID> {
        // Theres no guarrentee the state will still be there after we alter the palette
        // So we have to map the last state before modifying the palette
        let last_state_id = self.block_state_at(index);

        // If the state is already at `index` exit early
        if Some(state) == last_state_id {
            return None;
        }

        self.add_state_to_palette(state);

        self.set_state_internal(index, state);
        last_state_id
    }

    /// Removes all unused states from the palette
    ///
    /// If in indirect mode, adjusts the indexes in the [CompactStateBuffer] to compensate
    pub fn clean_palette(&mut self) {
        let states = if self.is_pal_direct {
            let mut states = self.palette.states().collect::<HashSet<_>>();
            self.states.iter().for_each(|u| {
                if states.contains(&(u as u16)) {
                    states.remove(&(u as u16));
                }
            });
            states
        } else {
            let mut states = HashSet::new();
            self.states.iter().for_each(|u| {
                if let Some(i) = self.palette.state_for(u) {
                    if !states.contains(&i) {
                        states.insert(i);
                    }
                }
            });
            states
        };

        for state in states {
            self.remove_state_from_palette(state);
        }
    }

    /// Sets the state at `index` to `state`
    ///
    /// Takes into account whether the palette is direct or indirect
    ///
    /// # Panics
    /// Panics if `state` is not already in the palette
    fn set_state_internal(&mut self, index: usize, state: StateID) {
        if self.is_pal_direct {
            self.states.set_nth_entry(index, state as usize);
        } else {
            self.states
                .set_nth_entry(index, self.palette.index_of(state).unwrap());
        }
    }

    /// Adds a state to the palette
    ///
    /// If we are in indirect mode we also update the indecies in the [CompactStateBuffer] to compensate
    fn add_state_to_palette(&mut self, state: StateID) {
        let index = match self.palette.insert(state) {
            InsertionResult::InsertedAndAltered {
                new_bits_per_block,
                index,
                ..
            } => {
                self.update_bits_per_block(new_bits_per_block);
                index
            }
            InsertionResult::Inserted { index } => index,
            _ => return,
        };

        if !self.is_pal_direct {
            self.states
                .alter(|u| if u >= index { Some(u + 1) } else { None });
        }
    }

    /// Removes a state from the palette
    ///
    /// If we are in indirect mode we also adjust indexes in the [CompactStateBuffer] to compensate
    fn remove_state_from_palette(&mut self, state: StateID) {
        let index = match self.palette.remove(state) {
            RemovalResult::RemovedAndAltered {
                index,
                new_bits_per_block,
                ..
            } => {
                self.update_bits_per_block(new_bits_per_block);
                index
            }
            RemovalResult::Removed { index } => index,
            RemovalResult::NotInPalette => return,
        };

        if !self.is_pal_direct {
            self.states
                .alter(|u| if u > index { Some(u - 1) } else { None })
        }
    }

    /// Updates the bits per block to the new value
    ///
    /// This involves updating the `is_pal_direct` variable and converting the [CompactStateBuffer] to be either direct or indircet
    fn update_bits_per_block(&mut self, bits_per_block: NonZeroU8) {
        self.states.modify_bits_per_entry(bits_per_block);
        // TODO: handle errors
        if bits_per_block.get() >= DIRECT_PALETTE_THRESHOLD && self.is_pal_direct {
            self.is_pal_direct = false;
            self.states.to_indirect_palette(&self.palette).unwrap();
        } else if bits_per_block.get() < DIRECT_PALETTE_THRESHOLD && !self.is_pal_direct {
            self.is_pal_direct = true;
            self.states.to_direct_palette(&self.palette).unwrap();
        }
    }

    pub fn lighting(&self) -> &Lighting {
        &self.lighting
    }

    pub fn gen_client_section(&self) -> ClientSection {
        let block_count = self.block_count() as i16;
        let bits_per_block = self.palette.bits_per_block().get();
        let palette = if self.is_pal_direct {
            None
        } else {
            Some(self.palette.states().map(|state| state as i32).collect())
        };
        let data = Box::<[u64]>::from(self.states.inner());

        ClientSection {
            block_count,
            bits_per_block,
            palette,
            data,
        }
    }

    pub fn into_packet_data(self) -> (ClientSection, Option<LightBuffer>, Option<LightBuffer>) {
        let block_count = self.block_count() as i16;
        let bits_per_block = self.palette.bits_per_block().get();
        let palette = if self.is_pal_direct {
            None
        } else {
            Some(
                self.palette
                    .index_to_state
                    .into_iter()
                    .map(|state| state as i32)
                    .collect(),
            )
        };
        let data = self.states.into_inner().into_boxed_slice();

        (
            ClientSection {
                block_count,
                bits_per_block,
                palette,
                data,
            },
            self.lighting.block,
            self.lighting.sky,
        )
    }

    #[inline]
    fn map_state_entry(&self, entry: usize) -> Option<StateID> {
        if !self.is_pal_direct {
            self.palette.state_for(entry)
        } else {
            Some(entry as StateID)
        }
    }
}

impl WriteToPacket for Section {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&(self.block_count() as i16));
        buffer.write_one(self.palette.bits_per_block().get());
        if !self.is_pal_direct {
            buffer.write_varying(&(self.palette.len() as i32));
            self.palette
                .states()
                .for_each(|state| buffer.write_varying(&(state as i32)));
        }
        let data = self.states.inner();
        buffer.write_varying(&(data.len() as i32));
        buffer.write_array(data);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectionY {
    pub raw: i8,
}

impl SectionY {
    #[inline]
    pub fn as_index(&self) -> usize {
        (self.raw + 1) as u8 as usize
    }
}

impl Display for SectionY {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.raw, f)
    }
}

impl Debug for SectionY {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

impl From<i8> for SectionY {
    fn from(raw: i8) -> Self {
        SectionY { raw }
    }
}

impl From<SectionY> for i8 {
    fn from(y: SectionY) -> Self {
        y.raw
    }
}

impl Serialize for SectionY {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_i8(self.raw)
    }
}

impl<'de> Deserialize<'de> for SectionY {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        Ok(SectionY {
            raw: Deserialize::deserialize(deserializer)?,
        })
    }
}

const OPT_SECTION_INDEX_NONE_NICHE: u8 = u8::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct OptionalSectionIndex {
    repr: u8,
}

impl OptionalSectionIndex {
    #[inline]
    const fn none() -> Self {
        OptionalSectionIndex {
            repr: OPT_SECTION_INDEX_NONE_NICHE,
        }
    }

    #[inline]
    fn some(index: usize) -> Self {
        if cfg!(debug_assertions) && index >= OPT_SECTION_INDEX_NONE_NICHE as usize {
            panic!(
                "Attempted to construct an optional section index with an illegal index: {}",
                index
            );
        }

        OptionalSectionIndex { repr: index as u8 }
    }

    #[inline]
    fn as_option(&self) -> Option<usize> {
        if self.repr != OPT_SECTION_INDEX_NONE_NICHE {
            Some(self.repr as usize)
        } else {
            None
        }
    }
}

pub struct SectionStore {
    section_mapping: [OptionalSectionIndex; MAX_SECTION_COUNT],
    sections: Vec<Section>,
}

impl SectionStore {
    pub fn new(size: usize) -> SectionStore {
        SectionStore {
            section_mapping: [OptionalSectionIndex::none(); MAX_SECTION_COUNT],
            sections: Vec::with_capacity(size),
        }
    }

    pub fn insert(&mut self, section: Section) -> Result<&mut Section, SectionInsertionError> {
        let index = match self.section_mapping.get_mut(section.y.as_index()) {
            Some(index) => index,
            None => return Err(SectionInsertionError::IndexOutOfRange(section.y)),
        };

        if index.as_option().is_some() {
            return Err(SectionInsertionError::AlreadyPresent(section.y));
        }

        *index = OptionalSectionIndex::some(self.sections.len());
        self.sections.push(section);
        Ok(self.sections.last_mut().unwrap())
    }

    pub fn get(&self, y: i8) -> Option<&Section> {
        self.section_mapping
            .get(SectionY::from(y).as_index())
            .and_then(OptionalSectionIndex::as_option)
            .map(|index| &self.sections[index])
    }

    pub fn get_mut(&mut self, y: i8) -> Option<&mut Section> {
        self.section_mapping
            .get(SectionY::from(y).as_index())
            .and_then(OptionalSectionIndex::as_option)
            .map(|index| &mut self.sections[index])
    }

    pub fn gen_bit_mask<F>(&self, include_boundary_sections: bool, mut f: F) -> BitMask
    where F: FnMut(&Section) -> bool {
        let sections = if include_boundary_sections {
            self.section_mapping.as_ref()
        } else {
            &self.section_mapping.as_ref()[1 .. self.section_mapping.len() - 1]
        };

        let mut mask = BitMask::new();
        for (raw_index, &map_index) in sections.iter().enumerate() {
            if let Some(map_index) = map_index.as_option() {
                if f(&self.sections[map_index]) {
                    mask.set(raw_index);
                }
            }
        }

        mask
    }

    pub fn into_packet_data(mut self) -> SectionAndLightData {
        let mut primary_bit_mask = BitMask::new();
        let mut block_light_mask = BitMask::new();
        let mut sky_light_mask = BitMask::new();

        self.sections.sort_by_key(|section| section.y);
        let max_idx = self.sections.last().map(|section| section.y.as_index());

        let mut sections = Vec::with_capacity(self.sections.len().max(2) - 2);
        let mut block_light = Vec::new();
        let mut sky_light = Vec::new();

        for section in self.sections {
            let index = section.y.as_index();
            let is_empty = section.is_empty();

            let (section, block, sky) = section.into_packet_data();

            if !is_empty && index > 0 && index < max_idx.unwrap() {
                primary_bit_mask.set(index - 1);
                sections.push(section);
            }

            if let Some(block) = block {
                block_light_mask.set(index);
                block_light.push(block);
            }

            if let Some(sky) = sky {
                sky_light_mask.set(index);
                sky_light.push(sky);
            }
        }

        SectionAndLightData {
            primary_bit_mask,
            sections: SectionData {
                sections: sections.into_boxed_slice(),
            },
            block_light_mask,
            sky_light_mask,
            empty_block_light_mask: block_light_mask.as_empty(),
            empty_sky_light_mask: sky_light_mask.as_empty(),
            block_light: block_light.into_boxed_slice(),
            sky_light: sky_light.into_boxed_slice(),
        }
    }

    // TODO: impl AsRef and things
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }
}

impl<'de> Deserialize<'de> for SectionStore {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        deserializer.deserialize_seq(SectionStoreVisitor)
    }
}

struct SectionStoreVisitor;

impl<'de> Visitor<'de> for SectionStoreVisitor {
    type Value = SectionStore;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "array of section")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where A: SeqAccess<'de> {
        let mut store = SectionStore::new(seq.size_hint().unwrap_or(0));

        while let Some(raw) = seq.next_element::<RawSection<'de>>()? {
            let section = Section::from_raw(raw).map_err(de::Error::custom)?;
            store.insert(section).map_err(de::Error::custom)?;
        }

        Ok(store)
    }
}

#[derive(Debug)]
pub enum SectionInsertionError {
    AlreadyPresent(SectionY),
    IndexOutOfRange(SectionY),
}

impl Display for SectionInsertionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyPresent(y) => write!(
                f,
                "attempted to insert a section at y={} where one was already present",
                y
            ),
            Self::IndexOutOfRange(y) => write!(
                f,
                "attempted to insert a section at y={} which is out of range (max {})",
                y, MAX_SECTION_COUNT
            ),
        }
    }
}

impl Error for SectionInsertionError {}

#[derive(Serialize, Deserialize)]
struct RawSection<'a> {
    #[serde(rename = "Y")]
    y: SectionY,
    #[serde(rename = "BlockLight")]
    block_light: Option<&'a [u8]>,
    #[serde(rename = "SkyLight")]
    sky_light: Option<&'a [u8]>,
    #[serde(rename = "Palette")]
    palette: Option<Vec<RawPaletteEntry<'a>>>,
    #[serde(rename = "BlockStates")]
    block_states: Option<Vec<i64>>,
}

#[derive(Serialize, Deserialize)]
struct RawPaletteEntry<'a> {
    // We get away with using a regular borrow here because we know that all these strings are
    // ASCII for valid chunk data
    #[serde(borrow, rename = "Name")]
    name: &'a UlnStr,
    #[serde(borrow, rename = "Properties", default = "HashMap::new")]
    properties: HashMap<&'a str, &'a str>,
}
