use crate::*;
use byteorder::{BigEndian, WriteBytesExt};
use flate2::{
    write::{GzEncoder, ZlibEncoder},
    Compression,
};
use std::io::{Error, ErrorKind, Result, Write};

impl NbtTag {
    fn id(&self) -> u8 {
        match self {
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
            NbtTag::LongArray(_) => 0xC,
        }
    }
}

/// Writes the given tag compound with the given name to the provided writer, writing only the raw
/// NBT data without any compression.
pub fn write_nbt_uncompressed<W>(
    writer: &mut W,
    root_name: &str,
    root: &NbtCompound,
) -> Result<()>
where
    W: Write,
{
    // Compound ID
    writer.write_u8(0xA)?;
    write_string(writer, root_name)?;
    write_compound(writer, root)
}

/// Wraps the given writer in a zlib encoder and then passes it to the uncompressed writer function.
pub fn write_nbt_zlib_compressed<W>(
    writer: &mut W,
    compression_level: Compression,
    root_name: &str,
    root: &NbtCompound,
) -> Result<()>
where
    W: Write,
{
    write_nbt_uncompressed(
        &mut ZlibEncoder::new(writer, compression_level),
        root_name,
        root,
    )
}

/// Wraps the given writer in a gz encoder and then passes it to the uncompressed writer function.
pub fn write_nbt_gz_compressed<W>(
    writer: &mut W,
    compression_level: Compression,
    root_name: &str,
    root: &NbtCompound,
) -> Result<()>
where
    W: Write,
{
    write_nbt_uncompressed(
        &mut GzEncoder::new(writer, compression_level),
        root_name,
        root,
    )
}

fn write_compound<W: Write>(writer: &mut W, compound: &NbtCompound) -> Result<()> {
    for (name, tag) in compound.as_ref().iter() {
        writer.write_u8(tag.id())?;
        write_string(writer, name)?;
        write_tag_body(writer, tag)?;
    }

    // TAG_End
    writer.write_u8(0)
}

fn write_tag_body<W: Write>(writer: &mut W, tag: &NbtTag) -> Result<()> {
    match tag {
        NbtTag::Byte(value) => writer.write_i8(*value),
        NbtTag::Short(value) => writer.write_i16::<BigEndian>(*value),
        NbtTag::Int(value) => writer.write_i32::<BigEndian>(*value),
        NbtTag::Long(value) => writer.write_i64::<BigEndian>(*value),
        NbtTag::Float(value) => writer.write_f32::<BigEndian>(*value),
        NbtTag::Double(value) => writer.write_f64::<BigEndian>(*value),
        NbtTag::ByteArray(value) => {
            writer.write_i32::<BigEndian>(value.len() as i32)?;

            for byte in value.iter() {
                writer.write_i8(*byte)?;
            }

            Ok(())
        }
        NbtTag::StringModUtf8(value) => write_string(writer, value),
        NbtTag::List(value) => {
            if value.is_empty() {
                // Five 0's indicates an empty list
                writer.write_all(&[0, 0, 0, 0, 0])
            } else {
                let type_id = value[0].id();
                writer.write_u8(type_id)?;
                writer.write_i32::<BigEndian>(value.len() as i32)?;

                for sub_tag in value.as_ref().iter() {
                    if sub_tag.id() != type_id {
                        return Err(Error::new(
                            ErrorKind::InvalidInput,
                            "Attempted to write NBT list with a non-homogenous type.",
                        ));
                    }

                    write_tag_body(writer, sub_tag)?;
                }

                Ok(())
            }
        }
        NbtTag::Compound(value) => write_compound(writer, value),
        NbtTag::IntArray(value) => {
            writer.write_i32::<BigEndian>(value.len() as i32)?;

            for int in value.iter() {
                writer.write_i32::<BigEndian>(*int)?;
            }

            Ok(())
        }
        NbtTag::LongArray(value) => {
            writer.write_i32::<BigEndian>(value.len() as i32)?;

            for long in value.iter() {
                writer.write_i64::<BigEndian>(*long)?;
            }

            Ok(())
        }
    }
}

fn write_string<W: Write>(writer: &mut W, string: &str) -> Result<()> {
    let mod_utf8 = cesu8::to_java_cesu8(string);
    writer.write_u16::<BigEndian>(mod_utf8.len() as u16)?;
    writer.write_all(&mod_utf8)
}
