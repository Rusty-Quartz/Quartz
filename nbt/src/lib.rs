#![warn(missing_docs)]
#![feature(try_trait)]

//! Provides support for encoding and decoding Minecraft's NBT format. This crate supports both
//! zlib and gz compression, and also provides tools for converting NBT to stringified NBT (SNBT).

mod repr;
mod tag;

/// Contains utilities for reading NBT data.
pub mod read;
/// Provides support for SNBT parsing.
pub mod snbt;
/// Contains utilities for writing NBT data.
pub mod write;

pub use repr::NbtRepr;
pub use tag::NbtCompound;
pub use tag::NbtList;
pub use tag::NbtTag;
