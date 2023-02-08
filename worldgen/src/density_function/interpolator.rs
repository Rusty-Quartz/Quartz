use dashmap::DashMap;
use qdat::world::location::BlockPosition;
use quartz_util::math::{floor_div, floor_mod, lerp_3d, LerpExt};
use std::sync::Arc;

use crate::density_function::{
    cache::Cacher,
    DensityFunctionContext,
    DensityFunctionContextProvider,
    DensityFunctionContextWrapper,
    DensityFunctionRef,
    DensityFunctionTree,
};

/// Manages the interpolation for a chunk
pub struct InterpolationManager {
    cell_count_xz: i32,
    cell_count_y: i32,
    in_cell_xyz: [i32; 3],
    cell_width: i32,
    cell_height: i32,
    array_index: usize,
    filling_cell: bool,
    cell_start_pos: BlockPosition,
    interpolators: Arc<DashMap<usize, CellInterpolator>>,
    cell_caches: Arc<DashMap<usize, CellCache>>,
    cache: Arc<Cacher>,
}

impl InterpolationManager {
    fn ctx(&self) -> NoiseDensityFunctionContext {
        NoiseDensityFunctionContext {
            pos: self.cell_start_pos
                + BlockPosition {
                    x: self.in_cell_xyz[0],
                    y: self.in_cell_xyz[1] as i16,
                    z: self.in_cell_xyz[2],
                },
            cache: self.cache.clone(),
            intepolators: InterpolatorAndCache {
                in_cell_xyz: self.in_cell_xyz,
                cell_width: self.cell_width,
                cell_height: self.cell_height,
                filling_cell: self.filling_cell,
                interpolators: self.interpolators.clone(),
                cell_caches: self.cell_caches.clone(),
            },
        }
    }
}

impl DensityFunctionContextProvider for InterpolationManager {
    fn for_index(&mut self, arr_index: usize) -> DensityFunctionContext {
        let z = floor_mod(arr_index as i32, self.cell_width);
        let val = floor_div(arr_index as i32, self.cell_width);
        let x = floor_mod(val, self.cell_width);
        let y = self.cell_height - 1 - floor_div(val, self.cell_width);

        self.in_cell_xyz[0] = x;
        self.in_cell_xyz[1] = y;
        self.in_cell_xyz[2] = z;
        self.array_index = arr_index;

        DensityFunctionContext::Noise(self.ctx())
    }

    fn fill_all_directly(
        &mut self,
        arr: &mut [f64],
        function: DensityFunctionRef,
        tree: Arc<DensityFunctionTree>,
    ) {
        self.array_index = 0;

        for y in (self.cell_height .. 0).rev() {
            self.in_cell_xyz[1] = y;
            for x in (self.cell_width .. 0).rev() {
                self.in_cell_xyz[0] = x;
                for z in (self.cell_width .. 0).rev() {
                    self.in_cell_xyz[2] = z;
                    let ctx = self.ctx();
                    let ctx = DensityFunctionContextWrapper {
                        ctx: DensityFunctionContext::Noise(ctx),
                        tree: tree.clone(),
                    };
                    arr[self.array_index] = function.calculate(&ctx)
                }
            }
        }
    }
}

pub struct NoiseDensityFunctionContext {
    pos: BlockPosition,
    cache: Arc<Cacher>,
    intepolators: InterpolatorAndCache,
}

impl NoiseDensityFunctionContext {
    pub fn get_pos(&self) -> BlockPosition {
        self.pos
    }

    pub fn get_cacher(&self) -> Option<&Cacher> {
        Some(&self.cache)
    }

    pub fn get_interpolator(&self) -> Option<&InterpolatorAndCache> {
        Some(&self.intepolators)
    }
}

pub struct InterpolatorAndCache {
    in_cell_xyz: [i32; 3],
    cell_width: i32,
    cell_height: i32,
    filling_cell: bool,
    interpolators: Arc<DashMap<usize, CellInterpolator>>,
    cell_caches: Arc<DashMap<usize, CellCache>>,
}

impl InterpolatorAndCache {
    pub fn interpolate(
        &self,
        id: usize,
        ctx: &DensityFunctionContextWrapper,
        child_func: DensityFunctionRef,
    ) -> f64 {
        match self.interpolators.get(&id) {
            Some(i) => i.calculate(
                self.filling_cell,
                self.in_cell_xyz,
                self.cell_width,
                self.cell_height,
            ),
            None => child_func.calculate(ctx),
        }
    }

    pub fn cache_all_in_cell(
        &self,
        id: usize,
        ctx: &DensityFunctionContextWrapper,
        child_func: DensityFunctionRef,
    ) -> f64 {
        match self.cell_caches.get(&id) {
            Some(cache) => cache.calculate(
                ctx,
                child_func,
                self.in_cell_xyz,
                self.cell_width,
                self.cell_height,
            ),
            None => child_func.calculate(ctx),
        }
    }
}

pub struct CellInterpolator {
    x_slice_0: Box<[Box<[f64]>]>,
    x_slice_1: Box<[Box<[f64]>]>,
    noise_xyz: [[[f64; 2]; 2]; 2],
    value_xz: [[f64; 2]; 2],
    value_z_0: f64,
    value_z_1: f64,
    value: f64,
}

impl CellInterpolator {
    fn new(cell_count_xz: usize, cell_count_y: usize) -> Self {
        let y = cell_count_y + 1;
        let xz = cell_count_xz + 1;
        CellInterpolator {
            x_slice_0: vec![vec![0.0; y].into_boxed_slice(); xz].into_boxed_slice(),
            x_slice_1: vec![vec![0.0; y].into_boxed_slice(); xz].into_boxed_slice(),
            noise_xyz: [[[0.0; 2]; 2]; 2],
            value_xz: [[0.0; 2]; 2],
            value_z_0: 0.0,
            value_z_1: 0.0,
            value: 0.0,
        }
    }

    fn select_cell_xz(&mut self, noise_y: usize, noise_z: usize) {
        self.noise_xyz[0][0][0] = self.x_slice_0[noise_z][noise_y];
        self.noise_xyz[0][0][1] = self.x_slice_0[noise_z + 1][noise_y];
        self.noise_xyz[0][1][0] = self.x_slice_0[noise_z][noise_y + 1];
        self.noise_xyz[0][1][1] = self.x_slice_0[noise_z + 1][noise_y + 1];
        self.noise_xyz[1][0][0] = self.x_slice_1[noise_z][noise_y];
        self.noise_xyz[1][0][1] = self.x_slice_1[noise_z + 1][noise_y];
        self.noise_xyz[1][1][0] = self.x_slice_1[noise_z][noise_y + 1];
        self.noise_xyz[1][1][1] = self.x_slice_1[noise_z + 1][noise_y + 1];
    }

    fn update_for_y(&mut self, delta_y: f64) {
        self.value_xz[0][0] = f64::lerp(delta_y, self.noise_xyz[0][0][0], self.noise_xyz[0][1][0]);
        self.value_xz[0][1] = f64::lerp(delta_y, self.noise_xyz[0][0][1], self.noise_xyz[0][1][1]);
        self.value_xz[1][0] = f64::lerp(delta_y, self.noise_xyz[1][0][0], self.noise_xyz[1][1][0]);
        self.value_xz[1][1] = f64::lerp(delta_y, self.noise_xyz[1][0][1], self.noise_xyz[1][1][1]);
    }

    fn update_for_x(&mut self, delta_x: f64) {
        self.value_z_0 = f64::lerp(delta_x, self.value_xz[0][0], self.value_xz[1][0]);
        self.value_z_1 = f64::lerp(delta_x, self.value_xz[0][1], self.value_xz[1][1]);
    }

    fn update_for_z(&mut self, delta_z: f64) {
        self.value = f64::lerp(delta_z, self.value_z_0, self.value_z_1);
    }

    fn swap_slices(&mut self) {
        std::mem::swap(&mut self.x_slice_0, &mut self.x_slice_1)
    }

    fn calculate(
        &self,
        filling_cell: bool,
        in_cell_xyz: [i32; 3],
        cell_width: i32,
        cell_height: i32,
    ) -> f64 {
        if filling_cell {
            lerp_3d(
                in_cell_xyz[0] as f64 / cell_width as f64,
                in_cell_xyz[1] as f64 / cell_height as f64,
                in_cell_xyz[2] as f64 / cell_width as f64,
                self.noise_xyz[0][0][0],
                self.noise_xyz[1][0][0],
                self.noise_xyz[0][1][0],
                self.noise_xyz[1][1][0],
                self.noise_xyz[0][0][1],
                self.noise_xyz[1][0][1],
                self.noise_xyz[0][1][1],
                self.noise_xyz[1][1][1],
            )
        } else {
            self.value
        }
    }

    fn fill_array(
        &self,
        arr: &mut [f64],
        filling_cell: bool,
        ctx_provider: &mut impl DensityFunctionContextProvider,
        interpolate_func_id: usize,
        wrapped_func: DensityFunctionRef,
        tree: Arc<DensityFunctionTree>,
    ) {
        if filling_cell {
            ctx_provider.fill_all_directly(arr, DensityFunctionRef(interpolate_func_id), tree)
        } else {
            ctx_provider.fill_all_directly(arr, wrapped_func, tree)
        }
    }
}

struct CellCache {
    values_yxz: Box<[f64]>,
}

impl CellCache {
    fn new(cell_width: usize, cell_height: usize) -> Self {
        CellCache {
            values_yxz: vec![0.0; cell_width * cell_width * cell_height].into_boxed_slice(),
        }
    }

    fn calculate(
        &self,
        ctx: &DensityFunctionContextWrapper,
        filler: DensityFunctionRef,
        in_cell_xyz: [i32; 3],
        cell_width: i32,
        cell_height: i32,
    ) -> f64 {
        let x = in_cell_xyz[0];
        let y = in_cell_xyz[1];
        let z = in_cell_xyz[2];

        if (0 .. cell_width).contains(&x)
            && (0 .. cell_height).contains(&y)
            && (0 .. cell_width).contains(&z)
        {
            self.values_yxz[(((cell_height - 1 - y) * cell_width + x) * cell_width + z) as usize]
        } else {
            filler.calculate(ctx)
        }
    }
}
