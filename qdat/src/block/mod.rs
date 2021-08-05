pub mod behavior;
#[allow(missing_docs, nonstandard_style, dead_code)]
pub mod states;

use crate::UnlocalizedName;
pub use behavior::BlockBehaviorSMT;
use std::fmt::{self, Debug, Display, Formatter};
use tinyvec::ArrayVec;

pub type StateID = u16;

/// A specific block type, not to be confused with a block state which specifies variants of a type. This
/// is used as a data handle for block states.
pub struct Block {
    /// The namespaced key identifying this block.
    pub name: UnlocalizedName,
    /// All block state properties and their valid values.
    pub properties: ArrayVec<[(String, Vec<String>); 16]>,
    /// The ID for the base state of this block.
    pub base_state: StateID,
    /// The ID for the default state of this block.
    pub default_state: StateID,
    /// The static method table defining the behavior of a block.
    pub behavior: BlockBehaviorSMT,
}

impl Display for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.name, f)
    }
}

impl Debug for Block {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
