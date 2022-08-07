pub trait DensityFunction {
    fn compute(context: impl FunctionContext) -> f64;
    fn fill_array(array: &mut [f64], context_provider: impl ContextProvider);
    fn map_all<I: DensityFunction, O: DensityFunction>(visitor: impl FnMut(I) -> O);
    fn min_value() -> f64;
    fn max_value() -> f64;
    // fn clamp()
}

pub trait FunctionContext {
    fn block_x() -> i32;
    fn block_y() -> i32;
    fn block_z() -> i32;

    // fn get_blender() -> Blender {};
}

pub trait ContextProvider {
    fn for_index<C: FunctionContext>(arr_index: i32) -> C;
    fn fill_all_directly(array: &mut [f64], density: impl DensityFunction);
}
