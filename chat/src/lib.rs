#![warn(missing_docs)]

//! Provides support for minecraft chat components.

/// Contains component builders for in-code component creation.
mod builder;
/// Defines and handles the application of chat colors.
pub mod color;
/// Defines chat components and their variants.
pub mod component;

/// Provides support for parsing a custom component format syntax.
#[macro_use]
pub mod cfmt;

pub use builder::TextComponentBuilder;
pub use component::Component;