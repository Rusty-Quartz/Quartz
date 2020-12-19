use crate::{base::registry::Registry, world::location::BlockPosition};

pub trait BlockBehavior<R: Registry> {
    // TODO: add world argument
    fn on_break(position: BlockPosition, state: &'static R::BlockState) {}
}

pub struct BlockBehaviorSMT<R: Registry> {
    on_break: fn(position: BlockPosition, state: &'static R::BlockState),
}

impl<R: Registry> BlockBehaviorSMT<R> {
    pub fn new<T: BlockBehavior<R>>() -> Self {
        BlockBehaviorSMT {
            on_break: T::on_break,
        }
    }
}

impl<R: Registry> Clone for BlockBehaviorSMT<R> {
    fn clone(&self) -> Self {
        BlockBehaviorSMT {
            on_break: self.on_break,
        }
    }
}

pub struct DefaultBehavior;

impl<R: Registry> BlockBehavior<R> for DefaultBehavior {}
