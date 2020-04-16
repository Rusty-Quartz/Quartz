use std::collections::HashMap;

pub enum NbtTag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    StringUtf8(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>)
}

impl NbtTag {
    pub fn id(&self) -> u8 {
        match self {
            End => 0x0,
            Byte(_) => 0x1
            Short(_) => 0x2,
            Int(_) => 0x3,
            Long(_) => 0x4,
            Float(_) => 0x5,
            Double(_) => 0x6,
            ByteArray(_) => 0x7,
            StringUtf8(_) => 0x8,
            List(_) => 0x9,
            Compound(_) => 0xA,
            IntArray(_) => 0xB,
            LongArray(_) => 0xC
        }
    }
}