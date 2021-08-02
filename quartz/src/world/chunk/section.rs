use super::{ChunkIoError, CompactStateBuffer, Lighting, Palette, DIRECT_PALETTE_THRESHOLD};
use crate::{
    block::{
        states::{is_air, AIR},
        BlockStateImpl,
        StateBuilder,
    },
    network::{
        packet::{ClientSection, SectionAndLightData},
        PacketBuffer,
        WriteToPacket,
    },
    BlockState,
    StateID,
};
use quartz_util::uln::UlnStr;
use serde::{
    de::{self, SeqAccess, Visitor},
    Deserialize,
    Serialize,
};
use std::{
    borrow::ToOwned,
    collections::HashMap,
    error::Error,
    fmt::{self, Debug, Display, Formatter},
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

    fn from_raw(raw: RawSection<'_>) -> Result<Self, ChunkIoError> {
        let (palette, states) = if raw.palette.is_none() || raw.block_states.is_none() {
            (Palette::new(), CompactStateBuffer::empty())
        } else {
            let mut palette = Palette::new();

            for palette_entry in raw.palette.unwrap() {
                let mut state = BlockState::builder(palette_entry.name).ok_or_else(|| {
                    ChunkIoError::UnknownBlockState(palette_entry.name.to_owned())
                })?;

                for (name, value) in palette_entry.properties {
                    state
                        .add_property(name, value)
                        .map_err(|msg| ChunkIoError::UnknownStateProperty(msg))?;
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
        self.states
            .iter()
            .flat_map(|entry| self.map_state_entry(entry))
            .filter(|&state| !is_air(state))
            .count()
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
            Some(
                self.palette
                    .states()
                    .map(|state| state as i32)
                    .collect::<Vec<i32>>()
                    .into_boxed_slice(),
            )
        };
        let data = Box::<[u64]>::from(self.states.inner());

        ClientSection {
            block_count,
            bits_per_block,
            palette,
            data,
        }
    }

    pub fn into_packet_data(self) -> SectionAndLightData {
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
                    .collect::<Vec<i32>>()
                    .into_boxed_slice(),
            )
        };
        let data = self.states.into_inner().into_boxed_slice();

        SectionAndLightData {
            section: ClientSection {
                block_count,
                bits_per_block,
                palette,
                data,
            },
            block_light: self.lighting.block,
            sky_light: self.lighting.sky,
        }
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
            .map(OptionalSectionIndex::as_option)
            .flatten()
            .map(|index| &self.sections[index])
    }

    pub fn gen_bit_mask<F>(&self, include_boundary_sections: bool, mut f: F) -> u128
    where F: FnMut(&Section) -> bool {
        let sections = if include_boundary_sections {
            self.section_mapping.as_ref()
        } else {
            &self.section_mapping.as_ref()[1 .. self.section_mapping.len() - 1]
        };

        let mut mask = 0;
        for &index in sections.iter().rev() {
            mask <<= 1;

            if let Some(index) = index.as_option() {
                if f(&self.sections[index]) {
                    mask |= 1;
                }
            }
        }

        mask
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
        let mut store = SectionStore {
            section_mapping: [OptionalSectionIndex::none(); MAX_SECTION_COUNT],
            sections: Vec::with_capacity(seq.size_hint().unwrap_or(0)),
        };

        loop {
            let raw: RawSection<'de> = match seq.next_element()? {
                Some(section) => section,
                None => break,
            };

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
