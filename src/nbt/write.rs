use crate::nbt::*;

impl NbtTag {
    pub fn id(&self) -> u8 {
        match self {
            NbtTag::End => 0x0,
            NbtTag::Byte(_) => 0x1,
            NbtTag::Short(_) => 0x2,
            NbtTag::Int(_) => 0x3,
            NbtTag::Long(_) => 0x4,
            NbtTag::Float(_) => 0x5,
            NbtTag::Double(_) => 0x6,
            NbtTag::ByteArray(_) => 0x7,
            NbtTag::StringModUtf8(_) => 0x8,
            NbtTag::List(_) => 0x9,
            NbtTag::Compound(_) => 0xA,
            NbtTag::IntArray(_) => 0xB,
            NbtTag::LongArray(_) => 0xC
        }
    }
}