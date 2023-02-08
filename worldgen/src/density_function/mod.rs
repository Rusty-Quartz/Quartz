mod density_functions;
use std::sync::Arc;

pub use density_functions::*;
use qdat::world::location::BlockPosition;

use crate::density_function::{
    cache::Cacher,
    interpolator::{InterpolatorAndCache, NoiseDensityFunctionContext},
};
pub mod cache;
pub mod interpolator;
pub mod spline;

#[derive(Clone)]
pub struct DensityFunctionTree {
    functions: Vec<DensityFunction>,
}

impl DensityFunctionTree {
    pub fn calculate(self: Arc<Self>, ctx: DensityFunctionContext) -> f64 {
        // Shouldn't be that expensive to clone here
        let start_function = self.functions[0].clone();

        let wrapper = DensityFunctionContextWrapper { ctx, tree: self };

        // Id is always 0 since id is just the index into the functions vec
        start_function.calculate(0, &wrapper)
    }
}

pub enum DensityFunctionContext {
    Noise(NoiseDensityFunctionContext),
    SinglePoint(BlockPosition),
}

impl DensityFunctionContext {
    /// Gets the position we're running the density function at
    pub fn get_pos(&self) -> BlockPosition {
        match self {
            DensityFunctionContext::Noise(n) => n.get_pos(),
            DensityFunctionContext::SinglePoint(pos) => *pos,
        }
    }

    /// Gets the cacher for the current chunk
    pub fn get_cacher(&self) -> Option<&Cacher> {
        match self {
            DensityFunctionContext::Noise(n) => n.get_cacher(),
            _ => None,
        }
    }

    /// Gets the interpolator for the current chunk
    pub fn get_interpolator(&self) -> Option<&InterpolatorAndCache> {
        match self {
            DensityFunctionContext::Noise(n) => n.get_interpolator(),
            _ => None,
        }
    }
}

pub struct DensityFunctionContextWrapper {
    ctx: DensityFunctionContext,
    tree: Arc<DensityFunctionTree>,
}

impl DensityFunctionContextWrapper {
    pub fn single_point(&self, pos: BlockPosition) -> DensityFunctionContextWrapper {
        DensityFunctionContextWrapper {
            ctx: DensityFunctionContext::SinglePoint(pos),
            tree: self.tree.clone(),
        }
    }

    pub fn get_pos(&self) -> BlockPosition {
        self.ctx.get_pos()
    }

    pub fn get_cacher(&self) -> Option<&Cacher> {
        self.ctx.get_cacher()
    }

    pub fn get_interpolator(&self) -> Option<&InterpolatorAndCache> {
        self.ctx.get_interpolator()
    }
}
pub trait DensityFunctionContextProvider {
    fn for_index(&mut self, arr_index: usize) -> DensityFunctionContext;
    fn fill_all_directly(
        &mut self,
        arr: &mut [f64],
        function: DensityFunctionRef,
        tree: Arc<DensityFunctionTree>,
    );
}

pub trait DensityFunctionVisitor {
    fn apply(&mut self, func: &mut DensityFunction);
    #[allow(unused_variables)]
    fn visit_noise(&self, noise: &mut NoiseHolder) {}
}
