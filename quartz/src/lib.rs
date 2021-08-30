#![deny(rust_2018_idioms)]
// We allow this because if we have modules with the same name, we don't publicly expose the inner one
#![allow(clippy::module_inception)]
// TODO: enable when ready
// #![warn(missing_docs)]
// when we enable warn missing_docs we will disable this
#![allow(clippy::missing_safety_doc)]
#![feature(new_uninit)]

//! This crate contains virtually all of the code, APIs, and other malarkey that makes quartz run. The server
//! code is launched through the separate `quartz_launcher` crate. Plugins should use this crate as a library

// Expose sub-crates
pub use quartz_chat as chat;
pub use quartz_nbt as nbt;
pub use quartz_util as util;

mod base;
/// Contains all relevant code to blocks and their implementations.
pub mod block;
/// Defines a brigadier-like command system for rust.
pub mod command;
/// Contains all relevant code to items and their implementations.
pub mod item;

/// Contains packet definitions and connection handlers.
pub mod network;
pub mod scheduler;
/// Contains world and chunk implementations, including chunk I/O utilities.
pub mod world;

pub use base::*;
