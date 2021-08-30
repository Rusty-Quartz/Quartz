// use crate::world::location::BlockPosition;

// pub trait BlockBehavior {
//     // TODO: add world argument
//     fn on_break(_position: BlockPosition, _state: &'static BlockState) {}
// }

pub struct BlockBehaviorSMT {
    // on_break: fn(position: BlockPosition, state: &'static BlockState),
}

impl BlockBehaviorSMT {
    // Allow new_without_default because we plan on yeeting this (or rewriting it) later anyway
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        BlockBehaviorSMT {
            // on_break: T::on_break,
        }
    }
}

impl Clone for BlockBehaviorSMT {
    fn clone(&self) -> Self {
        BlockBehaviorSMT {
            // on_break: self.on_break,
        }
    }
}

pub struct DefaultBehavior;

// impl BlockBehavior for DefaultBehavior {}
