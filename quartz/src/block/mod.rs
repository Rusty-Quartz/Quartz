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

use crate::base::registry::Registry;
use behavior::BlockBehaviorSMT;
use std::fmt::{self, Debug, Display, Formatter};
use tinyvec::ArrayVec;
use util::UnlocalizedName;

/// A specific block type, not to be confused with a block state which specifies variants of a type. This
/// is used as a data handle for block states.
pub struct Block<R: Registry> {
    /// The namespaced key identifying this block.
    pub name: UnlocalizedName,
    /// All block state properties and their valid values.
    pub properties: ArrayVec<[(String, Vec<String>); 16]>,
    /// The ID for the base state of this block.
    pub base_state: R::StateID,
    /// The ID for the default state of this block.
    pub default_state: R::StateID,
    /// The static method table defining the behavior of a block.
    pub behavior: BlockBehaviorSMT<R>,
}

impl<R: Registry> Display for Block<R> {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.name, f)
    }
}

impl<R: Registry> Debug for Block<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}
