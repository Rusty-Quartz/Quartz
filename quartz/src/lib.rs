#![deny(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

//! This crate contains virtually all of the code, APIs, and other malarkey that makes quartz run. The server
//! code is launched through the separate `quartz_launcher` crate. Plugins should use this crate as a library

// Expose sub-crates
pub use chat;
pub use nbt;
pub use util;

// Folders
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

// Files in src
/// Defines the server config.
pub mod config;
/// Main server module.
pub mod server;

pub use config::Config;
pub use quartz_plugins::Listeners;
pub use quartz_plugins::PluginInfo;
pub use quartz_plugins::plugin::plugin_info::get_quartz_info;
pub use server::QuartzServer;