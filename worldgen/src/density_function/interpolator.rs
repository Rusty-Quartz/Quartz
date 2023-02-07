pub trait Interpolator {
    fn cache_all_in_cell(&self, id: usize, arg: DensityFunctionRef) -> f64;
    fn interpolate(&self, id: usize, arg: DensityFunctionRef) -> f64;
}

pub struct ChunkNoiseInterpolator {}

pub struct CellInterpolator {
    // TODO
}
