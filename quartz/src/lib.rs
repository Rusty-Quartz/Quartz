#![deny(rust_2018_idioms)]
#![warn(missing_docs)]
#![feature(associated_type_bounds)]

//! This crate contains virtually all of the code, APIs, and other malarkey that makes quartz run. The server
//! code is launched through the separate `quartz_launcher` crate. Plugins should use this crate as a library

// Expose sub-crates
pub use chat;
pub use quartz_nbt as nbt;
pub use util;

mod base;
/// Contains all relevant code to blocks and their implementations.
pub mod block;
/// Defines a brigadier-like command system for rust.
pub mod command;
/// Contains all relevant code to items and their implementations.
pub mod item;
/// Contains packet definitions and connection handlers.
pub mod network;
/// Contains world and chunk implementations, including chunk I/O utilities.
pub mod world;

pub use base::*;
pub use quartz_plugins::{plugin::plugin_info::get_quartz_info, Listeners, PluginInfo};
