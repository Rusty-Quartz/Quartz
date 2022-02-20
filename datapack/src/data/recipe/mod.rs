pub mod cooking;
pub mod ingredient;
#[allow(clippy::module_inception)]
mod recipe;
pub mod shaped;
pub mod shapeless;
pub mod smithing;
pub mod stonecutting;

pub use recipe::*;
