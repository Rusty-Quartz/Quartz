use std::any::Any;
use nbt::NbtCompound;

// All block entities must impl this
/// Trait for block entities
pub trait BlockEntity: Any {
    /// Writes the entity info to a compound tag
    fn write_nbt(&self, nbt: &mut NbtCompound);
    /// Reads info from a compound tag
    fn from_nbt(&mut self, nbt: &NbtCompound);
    /// Ticks the block entity
    fn tick(&mut self);
}
