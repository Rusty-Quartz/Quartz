// TODO: we might want to change the name of these to something else
// depends on what we end up doing with StaticRegistry
pub mod registries;
pub(crate) mod static_registry;
// TODO: move behind a compiler flag or something
pub use super::static_registry::*;

/// Contains quartz assets data.
pub mod assets;
/// Defines the server config.
pub mod config;
pub mod diagnostic;
pub mod exec;
/// Main server module.
pub mod server;

pub use config::Config;
pub use diagnostic::Diagnostics;
pub use exec::*;
pub use server::QuartzServer;
