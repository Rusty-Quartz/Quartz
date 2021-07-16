use quartz_nbt::{NbtReprError, NbtStructureError};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io::Error as IOError,
};

#[derive(Debug)]
pub enum ChunkIoError {
    StdIo(IOError),
    Nbt(NbtReprError),
    InvalidNbtData(String),
}

impl Display for ChunkIoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ChunkIoError::StdIo(error) => Display::fmt(error, f),
            ChunkIoError::Nbt(error) => Display::fmt(error, f),
            ChunkIoError::InvalidNbtData(msg) => write!(f, "Invalid NBT Data: {}", msg),
        }
    }
}

impl Error for ChunkIoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ChunkIoError::StdIo(error) => Some(error),
            ChunkIoError::Nbt(error) => Some(error),
            ChunkIoError::InvalidNbtData(_) => None,
        }
    }
}

impl From<IOError> for ChunkIoError {
    fn from(x: IOError) -> Self {
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
