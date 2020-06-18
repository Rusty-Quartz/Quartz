use std::io::{Read, Result, Error, ErrorKind};
use byteorder::ReadBytesExt;
use byteorder::BigEndian;
use flate2::read::{ZlibDecoder, GzDecoder};
use crate::nbt::*;

pub fn read_nbt_uncompressed<R>(source: &mut R) -> Result<(NbtCompound, String)>
where
    R: Read
{
    let root_id = source.read_u8()?;
    if root_id != 0xA {
        return Err(Error::new(ErrorKind::InvalidData, "NBT data does not start with a compound type."));
    }

    let root_name = read_string(source)?;
    match read_tag_body(source, 0xA) {
        Ok(NbtTag::Compound(compound)) => Ok((compound, root_name)),
        Err(e) => Err(e),
        _ => unreachable!()
    }
}

pub fn read_nbt_zlib_compressed<R>(source: &mut R) -> Result<(NbtCompound, String)>
where
    R: Read
{
    read_nbt_uncompressed(&mut ZlibDecoder::new(source))
}

pub fn read_nbt_gz_compressed<R>(source: &mut R) -> Result<(NbtCompound, String)>
where
    R: Read
{
    read_nbt_uncompressed(&mut GzDecoder::new(source))
}

fn read_tag_body<R>(source: &mut R, id: u8) -> Result<NbtTag>
where
    R: Read
{
    let tag = match id {
        0x1 => NbtTag::Byte(source.read_i8()?),
        0x2 => NbtTag::Short(source.read_i16::<BigEndian>()?),
        0x3 => NbtTag::Int(source.read_i32::<BigEndian>()?),
        0x4 => NbtTag::Long(source.read_i64::<BigEndian>()?),
        0x5 => NbtTag::Float(source.read_f32::<BigEndian>()?),
        0x6 => NbtTag::Double(source.read_f64::<BigEndian>()?),
        0x7 => {
            let len = source.read_i32::<BigEndian>()? as usize;
            let mut array = vec![0_i8; len];

            for i in 0..len {
                array[i] = source.read_i8()?;
            }

            NbtTag::ByteArray(array)
        },
        0x8 => NbtTag::StringModUtf8(read_string(source)?),
        0x9 => {
            let type_id = source.read_u8()?;
            let len = source.read_i32::<BigEndian>()? as usize;

            // Make sure we don't have a list of TAG_End unless it's empty or an invalid type
            if type_id > 0xC || (type_id == 0 && len > 0) {
                return Err(Error::new(ErrorKind::InvalidData, "Invalid list type encountered."));
            }

            if len <= 0 {
                return Ok(NbtTag::List(NbtList::new()));
            }

            let mut list = NbtList::with_capacity(len);
            for _ in 0..len {
                list.add(read_tag_body(source, type_id)?);
            }

            NbtTag::List(list)
        },
        0xA => {
            let mut compound = NbtCompound::new();
            let mut tag_id = source.read_u8()?;

            // Read until TAG_End
            while tag_id != 0x0 {
                let name = read_string(source)?;
                let tag = read_tag_body(source, tag_id)?;
                compound.set(name, tag);
                tag_id = source.read_u8()?;
            }

            NbtTag::Compound(compound)
        },
        0xB => {
            let len = source.read_i32::<BigEndian>()? as usize;
            let mut array = vec![0_i32; len];

            for i in 0..len {
                array[i] = source.read_i32::<BigEndian>()?;
            }

            NbtTag::IntArray(array)
        },
        0xC => {
            let len = source.read_i32::<BigEndian>()? as usize;
            let mut array = vec![0_i64; len];

            for i in 0..len {
                array[i] = source.read_i64::<BigEndian>()?;
            }

            NbtTag::LongArray(array)
        },
        _ => return Err(Error::new(ErrorKind::InvalidData, "Invalid tag type encountered."))
    };

    Ok(tag)
}

fn read_string<R>(source: &mut R) -> Result<String>
where
    R: Read
{
    let len = source.read_u16::<BigEndian>()? as usize;
    let mut bytes = vec![0; len];
    source.read_exact(&mut bytes)?;
    
    let java_decoded = match cesu8::from_java_cesu8(&bytes) {
        Ok(string) => string,
        Err(_) => return Err(Error::new(ErrorKind::InvalidData, "Invalid string encoding."))
    };

    Ok(java_decoded.into_owned())
}