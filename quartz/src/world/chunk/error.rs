use crate::world::chunk::LightingInitError;
use quartz_nbt::{NbtReprError, NbtStructureError};
use quartz_util::uln::UnlocalizedName;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io::Error as IoError,
};

#[derive(Debug)]
pub enum ChunkIoError {
    StdIo(IoError),
    UnknownBlockState(UnlocalizedName),
    UnknownStateProperty(String),
    Lighting(LightingInitError),
    Nbt(NbtReprError),
    InvalidNbtData(String),
}

impl Display for ChunkIoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ChunkIoError::StdIo(error) => Display::fmt(error, f),
            ChunkIoError::UnknownBlockState(state) => write!(f, "Unknown block state {}", state),
            ChunkIoError::UnknownStateProperty(msg) => Display::fmt(msg, f),
            ChunkIoError::Lighting(error) => Display::fmt(error, f),
            ChunkIoError::Nbt(error) => Display::fmt(error, f),
            ChunkIoError::InvalidNbtData(msg) => write!(f, "Invalid NBT Data: {}", msg),
        }
    }
}

impl Error for ChunkIoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ChunkIoError::StdIo(error) => Some(error),
            ChunkIoError::Lighting(error) => Some(error),
            ChunkIoError::Nbt(error) => Some(error),
            _ => None,
        }
    }
}

impl From<IoError> for ChunkIoError {
    fn from(x: IoError) -> Self {
        ChunkIoError::StdIo(x)
    }
}

impl From<NbtReprError> for ChunkIoError {
    fn from(x: NbtReprError) -> Self {
        ChunkIoError::Nbt(x)
    }
}

impl From<NbtStructureError> for ChunkIoError {
    fn from(x: NbtStructureError) -> Self {
        ChunkIoError::Nbt(x.into())
    }
}

impl From<LightingInitError> for ChunkIoError {
    fn from(x: LightingInitError) -> Self {
        ChunkIoError::Lighting(x)
    }
}
