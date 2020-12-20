use crate::block::entities::FurnaceBlockEntity;
use enum_dispatch::enum_dispatch;
use quartz_nbt::NbtCompound;

// All block entities must impl this
/// Trait for block entities
#[enum_dispatch]
pub trait BlockEntity {
    /// Writes the entity info to a compound tag
    fn write_nbt(&self, nbt: &mut NbtCompound);
    /// Reads info from a compound tag
    fn from_nbt(&mut self, nbt: &NbtCompound);
    /// Ticks the block entity
    fn tick(&mut self);
}

#[enum_dispatch(BlockEntity)]
pub enum StaticBlockEntity {
    FurnaceBlockEntity,
}
