use crate::{base::BlockState, world::location::BlockPosition};

pub trait BlockBehavior {
    // TODO: add world argument
    fn on_break(_position: BlockPosition, _state: &'static BlockState) {}
}

pub struct BlockBehaviorSMT {
    on_break: fn(position: BlockPosition, state: &'static BlockState),
}

impl BlockBehaviorSMT {
    pub fn new<T: BlockBehavior>() -> Self {
        BlockBehaviorSMT {
            on_break: T::on_break,
        }
    }
}

impl Clone for BlockBehaviorSMT {
    fn clone(&self) -> Self {
        BlockBehaviorSMT {
            on_break: self.on_break,
        }
    }
}

pub struct DefaultBehavior;

impl BlockBehavior for DefaultBehavior {}
