#![warn(missing_docs)]

//! Provides generic utilities for quartz, the minecraft server implementation in
//! rust.

/// Configures log4rs to copy minecraft's logging style.
pub mod logging;
/// Contains optimized maps where hash maps are insufficient.
pub mod map;
mod uln;
mod uuid;

pub use uln::UnlocalizedName;
pub use uuid::Uuid;