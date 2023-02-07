use dashmap::DashMap;
use qdat::world::location::BlockPosition;

use crate::density_function::{
    DensityFunctionContext,
    DensityFunctionContextWrapper,
    DensityFunctionRef,
};

pub struct Cacher {
    cache_2d: DashMap<(usize, BlockPosition), f64>,
    flat_cache: DashMap<(usize, BlockPosition), f64>,
    cache_once: DashMap<usize, f64>,
}

impl Cacher {
    pub(super) fn cache_2d(
        &self,
        id: usize,
        child_func: &DensityFunctionRef,
        ctx: &DensityFunctionContextWrapper,
    ) -> f64 {
        let block_pos = ctx.get_pos();
        match self.cache_2d.get(&(id, block_pos)) {
            Some(val) => *val,
            None => {
                let val = child_func.calculate(ctx);
                self.cache_2d.insert((id, block_pos), val);
                val
            }
        }
    }

    pub(super) fn flat_cache(
        &self,
        id: usize,
        child_func: &DensityFunctionRef,
        ctx: &DensityFunctionContextWrapper,
    ) -> f64 {
        let block_pos = ctx.get_pos();

        match self.flat_cache.get(&(id, block_pos)) {
            Some(val) => *val,
            None => {
                let val = child_func.calculate(&ctx.single_point(block_pos));
                self.flat_cache.insert((id, block_pos), val);
                val
            }
        }
    }

    pub(super) fn cache_once(
        &self,
        id: usize,
        child_func: &DensityFunctionRef,
        ctx: &DensityFunctionContextWrapper,
    ) -> f64 {
        match self.cache_once.get(&id) {
            Some(val) => *val,
            None => {
                let val = child_func.calculate(ctx);
                self.cache_once.insert(id, val);
                val
            }
        }
    }
}
