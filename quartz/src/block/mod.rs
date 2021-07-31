pub mod behavior;
#[allow(missing_docs)]
pub mod entity;
pub(crate) mod init;
mod state;
#[allow(missing_docs, nonstandard_style, dead_code)]
pub mod states;

#[allow(missing_docs)]
pub mod entities {
    pub mod furnace_entity;
    pub use furnace_entity::FurnaceBlockEntity;
}

pub use state::*;

use crate::base::StateID;
use behavior::BlockBehaviorSMT;
use quartz_util::uln::UnlocalizedName;
use std::fmt::{self, Debug, Display, Formatter};
use tinyvec::ArrayVec;

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
