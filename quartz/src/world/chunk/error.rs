use quartz_nbt::{NbtReprError, NbtStructureError};
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    io::Error as IOError,
};

#[derive(Debug)]
pub enum ChunkIOError {
    IO(IOError),
    NbtStructure(NbtStructureError),
    InvalidNbtData(String),
}

impl Display for ChunkIOError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ChunkIOError::IO(error) => Display::fmt(error, f),
            ChunkIOError::NbtStructure(error) => Display::fmt(error, f),
            ChunkIOError::InvalidNbtData(msg) => write!(f, "Invalid NBT Data: {}", msg),
        }
    }
}

impl Error for ChunkIOError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ChunkIOError::IO(error) => Some(error),
            ChunkIOError::NbtStructure(error) => Some(error),
            ChunkIOError::InvalidNbtData(_) => None,
        }
    }
}

impl From<IOError> for ChunkIOError {
    fn from(x: IOError) -> Self {
        ChunkIOError::IO(x)
    }
}

impl From<NbtStructureError> for ChunkIOError {
    fn from(x: NbtStructureError) -> Self {
        ChunkIOError::NbtStructure(x)
    }
}

impl From<NbtReprError<NbtStructureError>> for ChunkIOError {
    fn from(x: NbtReprError<NbtStructureError>) -> Self {
        ChunkIOError::NbtStructure(x.into())
    }
}
