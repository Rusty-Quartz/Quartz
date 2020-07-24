use crate::block::{
    BlockState,
    StaticBlockState,
    entity::{BlockEntity, StaticBlockEntity}
};

static GLOBAL_STATIC_REGISTRY: StaticRegistry = StaticRegistry;

pub trait Registry
where
    Self:
        BlockRegistry +
        BlockEntityRegistry +
        Sized +
        'static
{
    fn new() -> Self;

    fn global() -> &'static Self;

    fn set_global(registry: Self) -> Result<(), Self>;
}

pub trait BlockRegistry {
    type BlockState: BlockState + Sized;
}

pub trait BlockEntityRegistry {
    type BlockEntity: BlockEntity + Send + Sized;
}

pub struct StaticRegistry;

impl BlockRegistry for StaticRegistry {
    type BlockState = StaticBlockState;
}

impl BlockEntityRegistry for StaticRegistry {
    type BlockEntity = StaticBlockEntity;
}

impl Registry for StaticRegistry {
    #[inline(always)]
    fn new() -> Self {
        StaticRegistry
    }

    #[inline(always)]
    fn global() -> &'static Self {
        &GLOBAL_STATIC_REGISTRY
    }

    #[inline(always)]
    fn set_global(_registry: Self) -> Result<(), Self> {
        Ok(())
    }
}
