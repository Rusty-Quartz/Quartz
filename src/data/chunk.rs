use crate::block::StateID;

pub struct Chunk {
    block_data: Box<[StateID; 4096]>
}

impl Chunk {
    #[inline(always)]
    fn index(x: i32, y: i32, z: i32) -> usize {
        (x + z * 16 + y * 256) as usize
    }

    #[inline]
    pub fn block_id(&self, x: i32, y: i32, z: i32) -> StateID {
        self.block_data[Self::index(x, y, z)]
    }
}