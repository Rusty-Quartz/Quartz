use std::sync::Arc;

use dashmap::DashMap;
use qdat::{registry::Resolvable, world::location::BlockPosition, UnlocalizedName};

use quartz_util::math::LerpExt;

use crate::{
    noise::{
        blended::BlendedNoise,
        normal::{NoiseParamteres, NormalNoise},
        simplex::SimplexNoise,
    },
    spline::{CustomCoordinate, SplineValue},
};

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

    fn get_blender(&self) -> Option<Blender> {
        self.ctx.get_blender()
    }

    fn get_cacher(&self) -> Option<&Cacher> {
        self.ctx.get_cacher()
    }
}

#[derive(Clone, Copy)]
pub struct DensityFunctionRef(usize);

impl DensityFunctionRef {
    pub(crate) fn calculate(&self, ctx: &DensityFunctionContextWrapper) -> f64 {
        ctx.tree.functions[self.0].calculate(self.0, ctx)
    }
}


// The only expensive part of this clone is the Arc clones
// and even then its not *that* bad
// so allowing clones should be fine
#[derive(Clone)]
pub enum DensityFunction {
    Abs {
        arg: DensityFunctionRef,
    },
    Add {
        a: DensityFunctionRef,
        b: DensityFunctionRef,
    },
    Beardifier,
    BlendAlpha,
    BlendDensity {
        arg: DensityFunctionRef,
    },
    BlendOffset,
    Cache2d {
        arg: DensityFunctionRef,
    },
    CacheAllInCell {
        arg: DensityFunctionRef,
    },
    CacheOnce {
        arg: DensityFunctionRef,
    },
    Clamp {
        arg: DensityFunctionRef,
        min: f64,
        max: f64,
    },
    Constant {
        arg: f64,
    },
    Cube {
        arg: DensityFunctionRef,
    },
    EndIslands {
        noise: Arc<SimplexNoise>,
    },
    FlatCache {
        arg: DensityFunctionRef,
    },
    HalfNegative {
        arg: DensityFunctionRef,
    },
    Interpolated {
        arg: DensityFunctionRef,
    },
    Max {
        a: DensityFunctionRef,
        b: DensityFunctionRef,
    },
    Min {
        a: DensityFunctionRef,
        b: DensityFunctionRef,
    },
    Mul {
        a: DensityFunctionRef,
        b: DensityFunctionRef,
    },
    Noise {
        noise: UnlocalizedName,
        xz_scale: f64,
        y_scale: f64,
    },
    OldBlendedNoise {
        noise: Arc<BlendedNoise>,
    },
    QuarterNegative {
        arg: DensityFunctionRef,
    },
    RangeChoice {
        arg: DensityFunctionRef,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: DensityFunctionRef,
        when_out_of_range: DensityFunctionRef,
    },
    Shift {
        offset_noise: NoiseHolder,
    },
    ShiftA {
        offset_noise: NoiseHolder,
    },
    ShiftB {
        offset_noise: NoiseHolder,
    },
    ShiftedNoise {
        noise: NoiseHolder,
        xz_scale: f64,
        y_scale: f64,
        shift_x: DensityFunctionRef,
        shift_y: DensityFunctionRef,
        shift_z: DensityFunctionRef,
    },
    Spline {
        spline: Arc<SplineValue<CustomCoordinate>>,
        min_value: f64,
        max_value: f64,
    },
    Square {
        arg: DensityFunctionRef,
    },
    Squeeze {
        arg: DensityFunctionRef,
    },
    WeirdScaledSampler {
        rarity_value_mapper: RarityValueMapper,
        noise: NoiseHolder,
        arg: DensityFunctionRef,
    },
    YClampedGradient {
        from_y: f64,
        to_y: f64,
        from_value: f64,
        to_value: f64,
    },
}
impl DensityFunction {
    pub fn calculate(&self, id: usize, ctx: &DensityFunctionContextWrapper) -> f64 {
        match self {
            DensityFunction::Abs { arg: argument } => argument.calculate(ctx).abs(),
            DensityFunction::Add {
                a: argument1,
                b: argument2,
            } => argument1.calculate(ctx) + argument2.calculate(ctx),
            DensityFunction::Beardifier => todo!(),
            DensityFunction::BlendAlpha => todo!(),
            DensityFunction::BlendDensity { arg: _argument } => todo!(),
            DensityFunction::BlendOffset => todo!(),
            DensityFunction::Cache2d { arg } => match ctx.get_cacher() {
                Some(cacher) => cacher.cache_2d(id, arg, ctx),
                None => arg.calculate(ctx),
            },
            DensityFunction::CacheAllInCell { arg: _argument } => todo!(),
            DensityFunction::CacheOnce { arg } => match ctx.get_cacher() {
                Some(cacher) => cacher.cache_once(id, arg, ctx),
                None => arg.calculate(ctx),
            },
            DensityFunction::Clamp {
                arg: input,
                min,
                max,
            } => input.calculate(ctx).clamp(*min, *max),
            DensityFunction::Constant { arg: argument } => *argument,
            DensityFunction::Cube { arg: argument } => {
                let val = argument.calculate(ctx);
                val * val * val
            }
            DensityFunction::EndIslands { noise: _noise } => todo!(),
            DensityFunction::FlatCache { arg } => match ctx.get_cacher() {
                Some(cacher) => cacher.flat_cache(id, arg, ctx),
                None => arg.calculate(ctx),
            },
            DensityFunction::HalfNegative { arg: argument } => {
                let val = argument.calculate(ctx);
                if val > 0.0 {
                    val
                } else {
                    val * 0.5
                }
            }
            DensityFunction::Interpolated { arg: _argument } => todo!(),
            DensityFunction::Max {
                a: argument1,
                b: argument2,
            } => argument1.calculate(ctx).max(argument2.calculate(ctx)),
            DensityFunction::Min { a, b } => a.calculate(ctx).min(b.calculate(ctx)),
            DensityFunction::Mul { a, b } => a.calculate(ctx) * b.calculate(ctx),
            DensityFunction::Noise {
                noise: _,
                xz_scale: _,
                y_scale: _,
            } => todo!(),
            DensityFunction::OldBlendedNoise { noise } => noise.calculate(ctx),
            DensityFunction::QuarterNegative { arg } => {
                let val = arg.calculate(ctx);
                if val > 0.0 {
                    val
                } else {
                    val * 0.25
                }
            }
            DensityFunction::RangeChoice {
                arg,
                min_inclusive,
                max_exclusive,
                when_in_range,
                when_out_of_range,
            } => {
                let val = arg.calculate(ctx);
                if (*min_inclusive .. *max_exclusive).contains(&val) {
                    when_in_range.calculate(ctx)
                } else {
                    when_out_of_range.calculate(ctx)
                }
            }
            DensityFunction::Shift { offset_noise } => {
                let pos = ctx.get_pos();
                compute_shifted_noise(offset_noise, pos.x as f64, pos.y as f64, pos.z as f64)
            }
            DensityFunction::ShiftA { offset_noise } => {
                let pos = ctx.get_pos();
                compute_shifted_noise(offset_noise, pos.x as f64, 0.0, pos.z as f64)
            }
            DensityFunction::ShiftB { offset_noise } => {
                let pos = ctx.get_pos();
                compute_shifted_noise(offset_noise, pos.z as f64, pos.x as f64, 0.0)
            }
            DensityFunction::ShiftedNoise {
                noise,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => {
                let pos = ctx.get_pos();
                let x = pos.x as f64 * *xz_scale + shift_x.calculate(ctx);
                let y = pos.y as f64 * *y_scale + shift_y.calculate(ctx);
                let z = pos.z as f64 * *xz_scale + shift_z.calculate(ctx);
                noise.get_value(x, y, z)
            }
            DensityFunction::Spline {
                spline,
                min_value,
                max_value,
            } => (spline.apply(ctx) as f64).clamp(*min_value, *max_value),
            DensityFunction::Square { arg } => {
                let val = arg.calculate(ctx);
                val * val
            }
            DensityFunction::Squeeze { arg } => {
                let val = arg.calculate(ctx);
                let clamped = val.clamp(-1.0, 1.0);
                clamped / 1.0 - clamped * clamped * clamped / 24.0
            }
            DensityFunction::WeirdScaledSampler {
                rarity_value_mapper,
                noise,
                arg,
            } => {
                let val = arg.calculate(ctx);

                let rarity = rarity_value_mapper.mapper(val);
                let pos = ctx.get_pos();
                rarity
                    * noise
                        .get_value(
                            pos.x as f64 / rarity,
                            pos.y as f64 / rarity,
                            pos.z as f64 / rarity,
                        )
                        .abs()
            }
            DensityFunction::YClampedGradient {
                from_y,
                to_y,
                from_value,
                to_value,
            } => {
                let pos = ctx.get_pos();
                LerpExt::clamped_map(pos.x as f64, *from_y, *to_y, *from_value, *to_value)
            }
        }
    }
}

pub trait DensityFunctionContext {
    /// Gets the position we're running the density function at
    fn get_pos(&self) -> BlockPosition;
    /// Gets the world blender for the region
    fn get_blender(&self) -> Option<Blender> {
        None
    }
    /// Gets the cacher for the current chunk
    fn get_cacher(&self) -> Option<&Cacher> {
        None
    }
    /// Gets the interpolator for the current chunk
    fn get_interpolator(&self) -> Option<()> {
        None
    }
}

pub trait Interpolator {
    fn cache_all_in_cell(&self, id: usize, arg: DensityFunctionRef) -> f64;
    fn interpolate(&self, id: usize, arg: DensityFunctionRef) -> f64;
}

pub struct SinglePointFunctionContext(BlockPosition);

impl DensityFunctionContext for SinglePointFunctionContext {
    fn get_pos(&self) -> BlockPosition {
        self.0
    }
}

pub trait DensityFunctionContextProvider<'a> {
    type Context: DensityFunctionContext + Clone;
    fn for_index(&self, arr_index: u32) -> Self::Context;
    fn fill_all_directly(&self, arr: &mut [f64], function: DensityFunction);
}

pub trait DensityFunctionVisitor {
    fn apply(func: &mut DensityFunction);
}

/// The world blender
///
/// A world blender will interpolate chunk data to make a smooth transition between chunks that use different terrain generation algorithms.
///
/// This is used in vanilla to smooth chunks generated in older versions with new chunks.
///
/// We currently do not implement this due to the main world gen algorithm not being completed.
pub struct Blender;

// Needed so that we can have spline::SamplePoint hold a DensityFunctionContext in one variant
// and otherwise should be unusable
impl DensityFunctionContext for () {
    fn get_pos(&self) -> BlockPosition {
        unreachable!("Unit type DensityFunctionContext cannot actually be used as a context")
    }

    fn get_blender(&self) -> Option<Blender> {
        unreachable!("Unit type DensityFunctionContext cannot actually be used as a context")
    }
}

fn compute_shifted_noise(normal_noise: &NoiseHolder, x: f64, y: f64, z: f64) -> f64 {
    normal_noise.get_value(x * 0.25, y * 0.25, z * 0.25) * 4.0
}

#[derive(Clone, Copy)]
pub enum RarityValueMapper {
    Type1,
    Type2,
}

impl RarityValueMapper {
    pub const fn name(&self) -> &'static str {
        match self {
            RarityValueMapper::Type1 => "type_1",
            RarityValueMapper::Type2 => "type_2",
        }
    }

    pub const fn max_rarity(&self) -> f64 {
        match self {
            RarityValueMapper::Type1 => 2.0,
            RarityValueMapper::Type2 => 3.0,
        }
    }

    pub fn mapper(&self, rarity: f64) -> f64 {
        match self {
            RarityValueMapper::Type1 => RarityValueMapper::get_spaghetti_rarity_3d(rarity),
            RarityValueMapper::Type2 => RarityValueMapper::get_spaghetti_rarity_2d(rarity),
        }
    }

    fn get_spaghetti_rarity_2d(rarity: f64) -> f64 {
        if rarity < -0.75 {
            0.5
        } else if rarity < -0.5 {
            0.75
        } else if rarity < 0.5 {
            1.0
        } else if rarity < 0.75 {
            2.0
        } else {
            3.0
        }
    }

    fn get_spaghetti_rarity_3d(rarity: f64) -> f64 {
        if rarity < -0.5 {
            0.75
        } else if rarity < 0.0 {
            1.0
        } else if rarity < 0.5 {
            1.5
        } else {
            2.0
        }
    }
}

/// Holds the parameters for a Noise
#[derive(Clone)]
pub struct NoiseHolder {
    pub noise_data: Resolvable<NoiseParamteres>,
    noise: Resolvable<NormalNoise>,
}

impl NoiseHolder {
    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        if let Some(n) = self.noise.get() {
            n.get_value(x, y, z)
        } else {
            0.0
        }
    }

    pub fn max_value(&self) -> f64 {
        if let Some(n) = self.noise.get() {
            n.max_value()
        } else {
            2.0
        }
    }
}

pub struct Cacher {
    cache_2d: DashMap<(usize, BlockPosition), f64>,
    flat_cache: DashMap<(usize, BlockPosition), f64>,
    cache_once: DashMap<usize, f64>,
}

impl Cacher {
    fn cache_2d(
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

    fn flat_cache(
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

    fn cache_once(
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
