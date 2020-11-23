use crate::block::{BlockState, StateBuilder};
use crate::world::location::{BlockPosition, ChunkCoordinatePair, CoordinatePair};
use crate::Registry;
use array_init::array_init;
use log::{error, warn};
use nbt::{NbtCompound, NbtList};
use num_traits::Zero;
use std::collections::HashMap;
use std::str::FromStr;
use util::{
    single_access::{AccessGuard, SingleAccessor},
    UnlocalizedName,
};

pub struct Chunk<R: Registry> {
    block_offset: CoordinatePair,
    sections: [Section<R::StateID>; 16],
    block_entities: HashMap<BlockPosition, SingleAccessor<R::BlockEntity>>,
}

impl<R: Registry> Chunk<R> {
    pub fn from_nbt(nbt: &NbtCompound) -> Option<Chunk<R>> {
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
            .get::<&NbtList>("Sections")?
            .iter_map::<_, &NbtCompound>()
        {
            // The raw palette which contains state information
            let raw_palette = match section?.get::<&NbtList>("Palette") {
                Some(palette) => palette,
                None => continue,
            };

            let mut palette = vec![R::StateID::zero(); raw_palette.len()].into_boxed_slice();
            let mut index: usize = 0;

            // Iterate over the block states in the palette
            for state in raw_palette.iter_map::<_, &NbtCompound>() {
                let state = state?;
                let state_name: &str = state.get("Name")?;

                // Initialize the state builder
                let mut state_builder =
                    match R::BlockState::builder(&UnlocalizedName::from_str(state_name).ok()?) {
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

    #[inline]
    fn section_index_absolute(&self, pos: BlockPosition) -> usize {
        ((pos.x - self.block_offset.x)
            + (pos.z - self.block_offset.z) * 16
            + (pos.y as i32 % 16) * 256) as usize
    }

    #[inline]
    pub fn block_id(&self, absolute_position: BlockPosition) -> R::StateID {
        match self.sections.get((absolute_position.y as usize) >> 4) {
            Some(section) => section.block_id(self.section_index_absolute(absolute_position)),
            None => R::StateID::zero(),
        }
    }

    #[inline]
    pub fn chunk_coordinates(&self) -> ChunkCoordinatePair {
        self.block_offset >> 4
    }

    pub fn block_entity_at(
        &self,
        absolute_position: BlockPosition,
    ) -> Option<AccessGuard<'_, R::BlockEntity>> {
        self.block_entities.get(&absolute_position)?.take()
    }
}

struct Section<T> {
    block_data: Option<Box<[T]>>,
    lighting: Option<Box<[u8]>>,
}

impl<T> Section<T> {
    const fn new() -> Self {
        Section {
            block_data: None,
            lighting: None,
        }
    }
}

impl<T: Zero + Copy> Section<T> {
    fn init(&mut self) {
        self.block_data = Some(vec![T::zero(); 4096].into_boxed_slice());
        self.lighting = Some(vec![0_u8; 2048].into_boxed_slice());
    }

    fn block_id(&self, index: usize) -> T {
        match self.block_data.as_ref() {
            Some(block_data) => block_data.get(index).map(|id| *id).unwrap_or(T::zero()),
            None => T::zero(),
        }
    }
}

impl<T> Default for Section<T> {
    fn default() -> Self {
        Self::new()
    }
}
