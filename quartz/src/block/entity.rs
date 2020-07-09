use nbt::NbtCompound;

// All block entities must impl this
/// Trait for block entities
pub trait BlockEntity {
    /// Writes the entity info to a compound tag
    fn write_nbt(&mut self, nbt: &mut NbtCompound);
    /// Reads info from a compound tag
    fn from_nbt(&mut self, nbt: &NbtCompound);
    /// Ticks the block entity
    fn tick(&mut self);
    /// The id of the block entity
    fn id(&self) -> BlockEntityType;
}

/// The different block entities
#[derive(PartialEq)]
pub enum BlockEntityType {
    Furnace
}