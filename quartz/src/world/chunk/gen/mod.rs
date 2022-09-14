#![allow(unused, clippy::too_many_arguments)]

use std::mem::MaybeUninit;

use qdat::world::location::{BlockPosition, Coordinate};
use quartz_nbt::NbtCompound;

use crate::world::chunk::{Section, SectionStore, MAX_SECTION_COUNT};

use ::noise::{NoiseFn, Perlin};

mod noise;
pub mod spline;
pub mod qnoise {
    use super::noise::*;
}
pub mod density_function;
pub mod random;

pub enum ChunkStatus {
    Empty,
    Shaping,
    Biomes,
    Features,
    Carving,
}

pub trait ChunkGenerator {
    fn start_chunk(coords: Coordinate) -> Self;
    fn shape_chunk(&mut self);
    fn finish_chunk(self) -> super::Chunk;
}

pub struct SimpleChunkGenerator {
    chunk: ProtoChunk,
    noise: Perlin,
}

impl ChunkGenerator for SimpleChunkGenerator {
    fn start_chunk(coords: Coordinate) -> Self {
        let chunk = ProtoChunk::new(coords.as_chunk());
        let noise = Perlin::new();

        Self { chunk, noise }
    }

    fn shape_chunk(&mut self) {
        let chunk = &mut self.chunk;
        for x in 0 .. 16 {
            for z in 0 .. 16 {
                let y = (self.noise.get([
                    (chunk.pos.as_block().x() + x) as f64 / 100.0,
                    (chunk.pos.as_block().z() + z) as f64 / 100.0,
                ]) * 40.0
                    + 60.0) as i16;
                // let y = 70;
                for i in 0 .. y {
                    let curr_y = y - i;
                    let section_index = curr_y >> 4;
                    let block_index =
                        chunk.section_index_absolute(BlockPosition { x, y: curr_y, z });
                    chunk
                        .sections
                        .get_mut(section_index as usize)
                        .unwrap()
                        .set_block_state_at(
                            block_index,
                            qdat::block::states::BlockStateData::Stone.id(),
                        );
                }
            }
        }
        chunk.state = ChunkState::Shaped;
    }

    fn finish_chunk(self) -> super::Chunk {
        self.chunk.into()
    }
}

pub enum ChunkState {
    Empty,
    Shaped,
    Done,
}

pub struct ProtoChunk {
    pub state: ChunkState,
    pub pos: Coordinate,
    pub sections: [Section; MAX_SECTION_COUNT],
    // TODO: figure out biomes
    pub biomes: Box<[i32]>,
}

impl ProtoChunk {
    pub fn new(pos: Coordinate) -> ProtoChunk {
        let mut sections: [MaybeUninit<Section>; MAX_SECTION_COUNT] =
            unsafe { MaybeUninit::uninit().assume_init() };

        for (i, s) in sections.iter_mut().enumerate() {
            s.write(Section::empty(i as i8 - 1));
        }

        let sections = unsafe { std::mem::transmute::<_, [Section; MAX_SECTION_COUNT]>(sections) };

        ProtoChunk {
            state: ChunkState::Empty,
            pos,
            sections,
            biomes: Box::new([0; 1024]),
        }
    }

    // x and z have to be in 0-16
    fn section_index_absolute(&self, pos: BlockPosition) -> usize {
        (pos.x + pos.z * 16 + (pos.y as i32 % 16) * 256) as usize
    }
}

#[allow(clippy::from_over_into)]
impl Into<super::Chunk> for ProtoChunk {
    fn into(self) -> super::Chunk {
        let chunk_size = self.sections.iter().filter(|s| !s.is_empty()).count();

        let mut section_store = SectionStore::new(chunk_size);

        for s in self.sections.into_iter().filter(|s| !s.is_empty()) {
            // assunme insertion cannot fail
            section_store.insert(s).unwrap();
        }


        super::Chunk::new(
            self.pos.as_block().into(),
            section_store,
            NbtCompound::new(),
            self.biomes,
        )
    }
}
