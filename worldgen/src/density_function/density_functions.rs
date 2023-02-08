use std::sync::Arc;

use qdat::registry::Resolvable;

use quartz_util::math::LerpExt;

use crate::{
    density_function::{
        spline::SplineValue,
        DensityFunctionContextWrapper,
        DensityFunctionTree,
        DensityFunctionVisitor,
    },
    noise::{
        blended::BlendedNoise,
        normal::{NoiseParamteres, NormalNoise},
        simplex::SimplexOctave,
    },
};


#[derive(Clone, Copy)]
pub struct DensityFunctionRef(pub(super) usize);

impl DensityFunctionRef {
    pub(crate) fn calculate(&self, ctx: &DensityFunctionContextWrapper) -> f64 {
        ctx.tree.functions[self.0].calculate(self.0, ctx)
    }
}

pub struct WrappedDensityFunction(usize, DensityFunction);

impl WrappedDensityFunction {
    pub fn calculate(&self, ctx: &DensityFunctionContextWrapper) -> f64 {
        self.1.calculate(self.0, ctx)
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
        noise: Arc<SimplexOctave>,
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
        noise: NoiseHolder,
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
        spline: Arc<SplineValue>,
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
            // TODO:
            // This seems to be the default value for the beardifier
            // How this will actually work is questionable
            // We don't yet support structures so the beardifier doesn't make sense to properly implement yet
            DensityFunction::Beardifier => 0.0,
            // TODO:
            // The Blend_ Functions require the Blender if we want proper values
            // These are the defaults for when there is no old terrain around the chunk
            // Since we are currently not targeting updating old worlds, we just give those
            DensityFunction::BlendAlpha => 1.0,
            DensityFunction::BlendDensity { arg } => arg.calculate(ctx),
            DensityFunction::BlendOffset => 0.0,
            DensityFunction::Cache2d { arg } => match ctx.get_cacher() {
                Some(cacher) => cacher.cache_2d(id, arg, ctx),
                None => arg.calculate(ctx),
            },
            DensityFunction::CacheAllInCell { arg } => match ctx.get_interpolator() {
                Some(i) => i.cache_all_in_cell(id, ctx, *arg),
                None => arg.calculate(ctx),
            },
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
            DensityFunction::EndIslands { noise } => {
                let pos = ctx.get_pos();
                (get_height_value(noise, pos.x / 8, pos.z / 8) as f64 - 8.0) / 128.0
            }
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
            DensityFunction::Interpolated { arg } => match ctx.get_interpolator() {
                Some(i) => i.interpolate(id, ctx, *arg),
                None => arg.calculate(ctx),
            },
            DensityFunction::Max {
                a: argument1,
                b: argument2,
            } => argument1.calculate(ctx).max(argument2.calculate(ctx)),
            DensityFunction::Min { a, b } => a.calculate(ctx).min(b.calculate(ctx)),
            DensityFunction::Mul { a, b } => a.calculate(ctx) * b.calculate(ctx),
            DensityFunction::Noise {
                noise,
                xz_scale,
                y_scale,
            } => {
                let pos = ctx.get_pos();
                noise.get_value(
                    pos.x as f64 * *xz_scale,
                    pos.y as f64 * *y_scale,
                    pos.z as f64 * *xz_scale,
                )
            }
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

impl DensityFunctionTree {
    pub fn map_all(&mut self, func: DensityFunctionRef, visitor: &mut impl DensityFunctionVisitor) {
        match &mut self.functions[func.0] {
            DensityFunction::Abs { arg }
            | DensityFunction::BlendDensity { arg }
            | DensityFunction::Cache2d { arg }
            | DensityFunction::CacheAllInCell { arg }
            | DensityFunction::CacheOnce { arg }
            | DensityFunction::Clamp { arg, .. }
            | DensityFunction::Cube { arg }
            | DensityFunction::FlatCache { arg }
            | DensityFunction::HalfNegative { arg }
            | DensityFunction::Interpolated { arg }
            | DensityFunction::QuarterNegative { arg }
            | DensityFunction::Square { arg }
            | DensityFunction::Squeeze { arg } => {
                let arg = *arg;
                self.map_all(arg, visitor);
            }
            DensityFunction::Add { a, b }
            | DensityFunction::Max { a, b }
            | DensityFunction::Min { a, b }
            | DensityFunction::Mul { a, b } => {
                let a = *a;
                let b = *b;
                self.map_all(a, visitor);
                self.map_all(b, visitor);
            }
            DensityFunction::Noise { noise, .. } => visitor.visit_noise(noise),
            DensityFunction::RangeChoice {
                arg,
                when_in_range,
                when_out_of_range,
                ..
            } => {
                let arg = *arg;
                let when_in_range = *when_in_range;
                let when_out_of_range = *when_out_of_range;
                self.map_all(arg, visitor);
                self.map_all(when_in_range, visitor);
                self.map_all(when_out_of_range, visitor);
            }
            DensityFunction::Shift { offset_noise }
            | DensityFunction::ShiftA { offset_noise }
            | DensityFunction::ShiftB { offset_noise } => visitor.visit_noise(offset_noise),
            DensityFunction::ShiftedNoise {
                noise,
                shift_x,
                shift_y,
                shift_z,
                ..
            } => {
                let shift_x = *shift_x;
                let shift_y = *shift_y;
                let shift_z = *shift_z;
                visitor.visit_noise(noise);
                self.map_all(shift_x, visitor);
                self.map_all(shift_y, visitor);
                self.map_all(shift_z, visitor);
            }
            DensityFunction::Spline { spline, .. } => {
                // Since we store spline in a Arc and all the DensityFunctions
                // and SplineValue only stores DensityFunctionRefs
                // we can clone and still have any changes be properly seen
                let spline = spline.clone();
                spline.map_all(self, visitor)
            }
            DensityFunction::WeirdScaledSampler { noise, arg, .. } => {
                visitor.visit_noise(noise);
                let arg = *arg;
                self.map_all(arg, visitor);
            }
            _ => {}
        }

        visitor.apply(&mut self.functions[func.0])
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

fn get_height_value(noise: &Arc<SimplexOctave>, x: i32, z: i32) -> f32 {
    let k = x / 2;
    let l = z / 2;
    let m = x % 2;
    let n = z % 2;
    let mut f = (100.0 - f32::sqrt((x * x + z * z) as f32) * 8.0).clamp(-100.0, 80.0);

    for o in -12 ..= 12 {
        for p in -12 ..= 12 {
            let q = (k + o) as i64;
            let r = (l + p) as i64;

            if q * q + r * r > 4096 && noise.sample_2d(q as f64, r as f64) < -0.9 {
                let g = ((q.abs() as f32) * 3439.0 + (r.abs() as f32) * 147.0) % 13.0 + 9.0;
                let h = (m - o * 2) as f32;
                let s = (n - p * 2) as f32;
                let t = (100.0 - f32::sqrt(h * h + s * s) * g).clamp(-100.0, 80.0);
                f = f.max(t)
            }
        }
    }

    f
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
