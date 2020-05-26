use crate::nbt::NbtCompound;
pub trait BlockEntity {
    fn write_nbt(&mut self, nbt: &mut NbtCompound);
    fn from_nbt(&mut self, nbt: &NbtCompound);
    fn tick(&mut self);
    fn id(&self) -> BlockEntityType;
}

#[derive(PartialEq)]
pub enum BlockEntityType {
    Furnace
}