mod density_functions;
use std::sync::Arc;

pub use density_functions::*;
use qdat::world::location::BlockPosition;

use crate::density_function::cache::Cacher;
pub mod cache;
pub mod spline;

#[derive(Clone)]
pub struct DensityFunctionTree {
    functions: Vec<DensityFunction>,
}

impl DensityFunctionTree {
    pub fn calculate<C: DensityFunctionContext + 'static>(&self, ctx: Arc<C>) -> f64 {
        // Shouldn't be that expensive to clone here
        let start_function = self.functions[0].clone();

        let wrapper = DensityFunctionContextWrapper { ctx, tree: self };

        // Id is always 0 since id is just the index into the functions vec
        start_function.calculate(0, &wrapper)
    }
}

pub trait DensityFunctionContext {
    /// Gets the position we're running the density function at
    fn get_pos(&self) -> BlockPosition;
    /// Gets the cacher for the current chunk
    fn get_cacher(&self) -> Option<&Cacher> {
        None
    }
    /// Gets the interpolator for the current chunk
    fn get_interpolator(&self) -> Option<()> {
        None
    }
}


#[derive(Clone)]
pub struct DensityFunctionContextWrapper<'a> {
    ctx: Arc<dyn DensityFunctionContext>,
    tree: &'a DensityFunctionTree,
}

impl<'a> DensityFunctionContextWrapper<'a> {
    pub fn single_point(&self, pos: BlockPosition) -> DensityFunctionContextWrapper<'a> {
        DensityFunctionContextWrapper {
            ctx: Arc::new(SinglePointFunctionContext(pos)),
            tree: self.tree,
        }
    }
}

impl<'a> DensityFunctionContext for DensityFunctionContextWrapper<'a> {
    fn get_pos(&self) -> BlockPosition {
        self.ctx.get_pos()
    }

    fn get_cacher(&self) -> Option<&Cacher> {
        self.ctx.get_cacher()
    }
}

pub struct SinglePointFunctionContext(BlockPosition);

impl DensityFunctionContext for SinglePointFunctionContext {
    fn get_pos(&self) -> BlockPosition {
        self.0
    }
}

pub trait DensityFunctionContextProvider {
    type Context: DensityFunctionContext + Clone;
    fn for_index(&self, arr_index: u32) -> Self::Context;
    fn fill_all_directly(&self, arr: &mut [f64], function: DensityFunction);
}

pub trait DensityFunctionVisitor {
    fn apply(func: &mut DensityFunction);
}
