use qdat::{
    world::{lighting::LightingInitError, location::Coordinate},
    UnlocalizedName,
};
use quartz_nbt::{io::NbtIoError, NbtReprError, NbtStructureError};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io::Error as IoError,
};

#[derive(Debug)]
pub enum ChunkDecodeError {
    StdIo(IoError),
    NbtIo(NbtIoError),
    NbtRepr(NbtReprError),
    UnknownBlockState(UnlocalizedName),
    UnknownStateProperty(String),
    Lighting(LightingInitError),
    ChunkRegionDesync(Coordinate),
    UnknownCompression(u8),
}

impl Display for ChunkDecodeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ChunkDecodeError::StdIo(error) => Display::fmt(error, f),
            ChunkDecodeError::NbtIo(error) => Display::fmt(error, f),
            ChunkDecodeError::NbtRepr(error) => Display::fmt(error, f),
            ChunkDecodeError::UnknownBlockState(state) =>
                write!(f, "Unknown block state {state}"),
            ChunkDecodeError::UnknownStateProperty(msg) => Display::fmt(msg, f),
            ChunkDecodeError::Lighting(error) => Display::fmt(error, f),
            ChunkDecodeError::ChunkRegionDesync(coords) =>
                write!(f, "Attempted to load chunk outside of region at {coords}"),
            ChunkDecodeError::UnknownCompression(id) => write!(
                f,
                "Encountered unknown compression scheme {id}, expected 1 or 2"
            ),
        }
    }
}

impl Error for ChunkDecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ChunkDecodeError::StdIo(error) => Some(error),
            ChunkDecodeError::NbtIo(error) => Some(error),
            ChunkDecodeError::Lighting(error) => Some(error),
            ChunkDecodeError::NbtRepr(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IoError> for ChunkDecodeError {
    fn from(x: IoError) -> Self {
        ChunkDecodeError::StdIo(x)
    }
}

impl From<NbtIoError> for ChunkDecodeError {
    fn from(x: NbtIoError) -> Self {
        ChunkDecodeError::NbtIo(x)
    }
}

impl From<NbtReprError> for ChunkDecodeError {
    fn from(x: NbtReprError) -> Self {
        ChunkDecodeError::NbtRepr(x)
    }
}

impl From<NbtStructureError> for ChunkDecodeError {
    fn from(x: NbtStructureError) -> Self {
        ChunkDecodeError::NbtRepr(x.into())
    }
}

impl From<LightingInitError> for ChunkDecodeError {
    fn from(x: LightingInitError) -> Self {
        ChunkDecodeError::Lighting(x)
    }
}
