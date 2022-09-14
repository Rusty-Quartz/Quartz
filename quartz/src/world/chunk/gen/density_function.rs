use qdat::{
    world::location::{BlockPosition, Coordinate},
    UnlocalizedName,
};
use quartz_datapack::data::{
    density_function::TerrainShaperSplineType,
    noise_settings::NoiseOptions,
};
use quartz_util::math::LerpExt;

use super::{
    noise::{
        blended::BlendedNoise,
        normal::{NoiseParamteres, NormalNoise},
        simplex::SimplexNoise,
    },
    spline::{CustomCoordinate, CustomPoint, SplineValue, TerrainShaper},
};

pub struct CompiledDensityFunction<'a, C: DensityFunctionContext + Clone> {
    function: DensityFunction<'a, C>,
}

impl<'a, C: DensityFunctionContext + Clone> CompiledDensityFunction<'a, C> {
    pub fn calculate(&mut self, ctx: &'a C) {
        self.function.calculate(ctx);
    }
}

#[derive(Clone)]
pub(super) enum DensityFunction<'a, C: DensityFunctionContext + Clone> {
    Abs {
        arg: Box<DensityFunction<'a, C>>,
    },
    Add {
        a: Box<DensityFunction<'a, C>>,
        b: Box<DensityFunction<'a, C>>,
    },
    Beardifier,
    BlendAlpha,
    BlendDensity {
        arg: Box<DensityFunction<'a, C>>,
    },
    BlendOffset,
    Cache2d {
        last_pos_2d: i64,
        last_value: f64,
        arg: Box<DensityFunction<'a, C>>,
    },
    CacheAllInCell {
        arg: Box<DensityFunction<'a, C>>,
    },
    CacheOnce {
        arg: Box<DensityFunction<'a, C>>,
    },
    Clamp {
        arg: Box<DensityFunction<'a, C>>,
        min: f64,
        max: f64,
    },
    Constant {
        arg: f64,
    },
    Cube {
        arg: Box<DensityFunction<'a, C>>,
    },
    EndIslands {
        noise: SimplexNoise,
    },
    FlatCache {
        arg: Box<DensityFunction<'a, C>>,
    },
    HalfNegative {
        arg: Box<DensityFunction<'a, C>>,
    },
    Interpolated {
        arg: Box<DensityFunction<'a, C>>,
    },
    Max {
        a: Box<DensityFunction<'a, C>>,
        b: Box<DensityFunction<'a, C>>,
    },
    Min {
        a: Box<DensityFunction<'a, C>>,
        b: Box<DensityFunction<'a, C>>,
    },
    Mul {
        a: Box<DensityFunction<'a, C>>,
        b: Box<DensityFunction<'a, C>>,
    },
    Noise {
        noise: UnlocalizedName,
        xz_scale: f64,
        y_scale: f64,
    },
    OldBlendedNoise {
        noise: BlendedNoise,
    },
    QuarterNegative {
        arg: Box<DensityFunction<'a, C>>,
    },
    RangeChoice {
        arg: Box<DensityFunction<'a, C>>,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: Box<DensityFunction<'a, C>>,
        when_out_of_range: Box<DensityFunction<'a, C>>,
    },
    Shift {
        noise_data: NoiseParamteres,
        offset_noise: Option<NormalNoise>,
    },
    ShiftA {
        noise_data: NoiseParamteres,
        offset_noise: Option<NormalNoise>,
    },
    ShiftB {
        noise_data: NoiseParamteres,
        offset_noise: Option<NormalNoise>,
    },
    ShiftedNoise {
        noise: Option<NormalNoise>,
        noise_params: NoiseParamteres,
        xz_scale: f64,
        y_scale: f64,
        shift_x: Box<DensityFunction<'a, C>>,
        shift_y: Box<DensityFunction<'a, C>>,
        shift_z: Box<DensityFunction<'a, C>>,
    },
    Slide {
        settings: Option<NoiseOptions>,
        arg: Box<DensityFunction<'a, C>>,
    },
    Spline {
        spline: SplineValue<CustomCoordinate<'a, C>>,
        min_value: f64,
        max_value: f64,
    },
    Square {
        arg: Box<DensityFunction<'a, C>>,
    },
    Squeeze {
        arg: Box<DensityFunction<'a, C>>,
    },
    TerrainShaperSpline {
        shaper: Option<TerrainShaper>,
        spline: TerrainShaperSplineType,
        min_value: f64,
        max_value: f64,
        continentalness: Box<DensityFunction<'a, C>>,
        erosion: Box<DensityFunction<'a, C>>,
        weirdness: Box<DensityFunction<'a, C>>,
    },
    WeirdScaledSampler {
        rarity_value_mapper: RarityValueMapper,
        noise: Option<NormalNoise>,
        noise_data: NoiseParamteres,
        arg: Box<DensityFunction<'a, C>>,
    },
    YClampedGradient {
        from_y: f64,
        to_y: f64,
        from_value: f64,
        to_value: f64,
    },
}
impl<'a, C: DensityFunctionContext + Clone> DensityFunction<'a, C> {
    pub fn calculate(&mut self, ctx: &'a C) -> f64 {
        match self {
            DensityFunction::Abs { arg: argument } => argument.calculate(ctx).abs(),
            DensityFunction::Add {
                a: argument1,
                b: argument2,
            } => argument1.calculate(ctx) + argument2.calculate(ctx),
            DensityFunction::Beardifier => todo!(),
            DensityFunction::BlendAlpha => todo!(),
            DensityFunction::BlendDensity { arg: argument } => todo!(),
            DensityFunction::BlendOffset => todo!(),
            DensityFunction::Cache2d {
                last_pos_2d,
                last_value,
                arg: argument,
            } => {
                let coord: Coordinate = ctx.get_pos().into();
                let curr_pos = coord.as_chunk_long();
                if curr_pos == *last_pos_2d {
                    *last_value
                } else {
                    let val = argument.calculate(ctx);
                    *last_value = val;
                    *last_pos_2d = curr_pos;
                    val
                }
            }
            DensityFunction::CacheAllInCell { arg: argument } => todo!(),
            DensityFunction::CacheOnce { arg: argument } => todo!(),
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
            DensityFunction::EndIslands { noise } => todo!(),
            DensityFunction::FlatCache { arg: argument } => todo!(),
            DensityFunction::HalfNegative { arg: argument } => {
                let val = argument.calculate(ctx);
                if val > 0.0 {
                    val
                } else {
                    val * 0.5
                }
            }
            DensityFunction::Interpolated { arg: argument } => todo!(),
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
            DensityFunction::Shift {
                noise_data,
                offset_noise,
            } => {
                let pos = ctx.get_pos();
                compute_shift_noise(offset_noise, pos.x as f64, pos.y as f64, pos.z as f64)
            }
            DensityFunction::ShiftA {
                noise_data,
                offset_noise,
            } => {
                let pos = ctx.get_pos();
                compute_shift_noise(offset_noise, pos.x as f64, 0.0, pos.z as f64)
            }
            DensityFunction::ShiftB {
                noise_data,
                offset_noise,
            } => {
                let pos = ctx.get_pos();
                compute_shift_noise(offset_noise, pos.z as f64, pos.x as f64, 0.0)
            }
            DensityFunction::ShiftedNoise {
                noise,
                noise_params,
                xz_scale,
                y_scale,
                shift_x,
                shift_y,
                shift_z,
            } => match noise {
                None => 0.0,
                Some(noise) => {
                    let pos = ctx.get_pos();
                    let x = pos.x as f64 * *xz_scale + shift_x.calculate(ctx);
                    let y = pos.y as f64 * *y_scale + shift_y.calculate(ctx);
                    let z = pos.z as f64 * *xz_scale + shift_z.calculate(ctx);
                    noise.get_value(x, y, z)
                }
            },
            DensityFunction::Slide { arg, settings } => match settings {
                Some(settings) => todo!(),
                None => arg.calculate(ctx),
            },
            DensityFunction::Spline {
                spline,
                min_value,
                max_value,
            } => (spline.apply(&CustomPoint(ctx)) as f64).clamp(*min_value, *max_value),
            DensityFunction::Square { arg } => {
                let val = arg.calculate(ctx);
                val * val
            }
            DensityFunction::Squeeze { arg } => {
                let val = arg.calculate(ctx);
                let clamped = val.clamp(-1.0, 1.0);
                clamped / 1.0 - clamped * clamped * clamped / 24.0
            }
            DensityFunction::TerrainShaperSpline {
                shaper,
                spline,
                min_value,
                max_value,
                continentalness,
                erosion,
                weirdness,
            } => match shaper {
                Some(shaper) => match spline {
                    TerrainShaperSplineType::Factor => (shaper.factor(&TerrainShaper::make_point(
                        continentalness.calculate(ctx) as f32,
                        erosion.calculate(ctx) as f32,
                        weirdness.calculate(ctx) as f32,
                    )) as f64)
                        .clamp(*min_value, *max_value),
                    TerrainShaperSplineType::Jaggedness =>
                        (shaper.jaggedness(&TerrainShaper::make_point(
                            continentalness.calculate(ctx) as f32,
                            erosion.calculate(ctx) as f32,
                            weirdness.calculate(ctx) as f32,
                        )) as f64)
                            .clamp(*min_value, *max_value),
                    TerrainShaperSplineType::Offset => (shaper.offset(&TerrainShaper::make_point(
                        continentalness.calculate(ctx) as f32,
                        erosion.calculate(ctx) as f32,
                        weirdness.calculate(ctx) as f32,
                    )) as f64)
                        .clamp(*min_value, *max_value),
                },
                None => 0.0,
            },
            DensityFunction::WeirdScaledSampler {
                rarity_value_mapper,
                noise,
                noise_data,
                arg,
            } => match noise {
                None => 0.0,
                Some(noise) => {
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
            },
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
    fn get_pos(&self) -> BlockPosition;
    fn get_blender(&self) -> Blender;
}

pub trait DensityFunctionContextProvider<'a> {
    type Context: DensityFunctionContext + Clone;
    fn for_index(&self, arr_index: u32) -> Self::Context;
    fn fill_all_directly(
        &self,
        arr: &mut [f64],
        function: CompiledDensityFunction<'a, Self::Context>,
    );
}

// TODO: move to its own file with a proper impl
pub struct Blender;

// Needed so that we can have spline::SamplePoint hold a DensityFunctionContext in one variant
// and not otherwise
impl DensityFunctionContext for () {
    fn get_pos(&self) -> BlockPosition {
        unreachable!("Unit type DensityFunctionContext cannot actually be used as a context")
    }

    fn get_blender(&self) -> Blender {
        unreachable!("Unit type DensityFunctionContext cannot actually be used as a context")
    }
}

fn compute_shift_noise(normal_noise: &Option<NormalNoise>, x: f64, y: f64, z: f64) -> f64 {
    match normal_noise {
        Some(noise) => noise.get_value(x * 0.25, y * 0.25, z * 0.25) * 4.0,
        None => 0.0,
    }
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
