use crate::{PacketBuffer, PacketSerdeError, ReadFromPacket, WriteToPacket};
use log::warn;

#[derive(Clone, Copy, Debug)]
pub struct BitMask(u128);

impl BitMask {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn into_raw(self) -> u128 {
        self.0
    }

    pub fn set(&mut self, index: usize) {
        self.0 |= 1u128 << index;
    }

    pub fn as_empty(&self) -> Self {
        Self(!self.0 | 1)
    }
}

impl ReadFromPacket for BitMask {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let len = buffer.read_varying::<i32>()? as usize;

        if len > 2 {
            warn!("Encountered bit mask containing more than 128 bits");
        }

        let mut mask = 0;
        for _ in 0 .. len.min(2) {
            mask <<= 64;
            mask |= buffer.read::<u64>()? as u128;
        }

        Ok(Self(mask))
    }
}

impl WriteToPacket for BitMask {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let lo = (self.0 & u64::MAX as u128) as u64;
        let hi = (self.0 >> 64) as u64;

        if hi == 0 {
            buffer.write_varying(&1i32);
            buffer.write(&lo);
        } else {
            buffer.write_varying(&2i32);
            buffer.write(&lo);
            buffer.write(&hi);
        };
    }
}
