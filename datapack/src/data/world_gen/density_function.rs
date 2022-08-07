use qdat::UnlocalizedName;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum DensityFunctionProvider {
    Constant(f64),
    Inline(Box<DensityFunction>),
    Reference(UnlocalizedName),
}


#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DensityFunction {
    #[serde(rename = "minecraft:abs")]
    Abs { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:add")]
    Add {
        argument1: DensityFunctionProvider,
        argument2: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:beardifier")]
    Beardifier,
    #[serde(rename = "minecraft:blend_alpha")]
    BlendAlpha,
    #[serde(rename = "minecraft:blend_density")]
    BendDensity { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:blend_offset")]
    BlendOffset,
    #[serde(rename = "minecraft:cache_2d")]
    Cache2d { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:cache_all_in_cell")]
    CacheAllInCell { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:cache_once")]
    CacheOnce { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:clamp")]
    Clamp {
        input: DensityFunctionProvider,
        min: f64,
        max: f64,
    },
    #[serde(rename = "minecraft:constant")]
    Constant { argument: f64 },
    #[serde(rename = "minecraft:cube")]
    Cube { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:end_islands")]
    EndIslands,
    #[serde(rename = "minecraft:flat_cache")]
    FlatCache { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:half_negative")]
    HalfNegative { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:interpolated")]
    Interpolated { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:max")]
    Max {
        argument1: DensityFunctionProvider,
        argument2: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:min")]
    Min {
        argument1: DensityFunctionProvider,
        argument2: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:mul")]
    Mul {
        argument1: DensityFunctionProvider,
        argument2: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:noise")]
    Noise {
        noise: UnlocalizedName,
        xz_scale: f64,
        y_scale: f64,
    },
    #[serde(rename = "minecraft:old_blended_noise")]
    OldBlendedNoise,
    /*{
        xz_scale: f64,
        y_scale: f64,
        xz_factor: f64,
        y_factor: f64,
        smear_scale_multiplier: f64,
    } */
    #[serde(rename = "minecraft:quarter_negative")]
    QuarterNegative { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:range_choice")]
    RangeChoice {
        input: DensityFunctionProvider,
        min_inclusive: f64,
        max_exclusive: f64,
        when_in_range: DensityFunctionProvider,
        when_out_of_range: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:shift")]
    Shift { argument: UnlocalizedName },
    #[serde(rename = "minecraft:shift_a")]
    ShiftA { argument: UnlocalizedName },
    #[serde(rename = "minecraft:shift_b")]
    ShiftB { argument: UnlocalizedName },
    #[serde(rename = "minecraft:shifted_noise")]
    ShiftedNoise {
        noise: UnlocalizedName,
        xz_scale: f64,
        y_scale: f64,
        shift_x: DensityFunctionProvider,
        shift_y: DensityFunctionProvider,
        shift_z: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:slide")]
    Slide { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:spline")]
    Spline {
        spline: SplineValue,
        min_value: f64,
        max_value: f64,
    },
    #[serde(rename = "minecraft:square")]
    Square { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:squeeze")]
    Squeeze { argument: DensityFunctionProvider },
    #[serde(rename = "minecraft:terrain_shaper_spline")]
    TerrainShaperSpline {
        spline: TerrainShaperSplineType,
        min_value: f64,
        max_value: f64,
        continentalness: DensityFunctionProvider,
        erosion: DensityFunctionProvider,
        weirdness: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:weird_scaled_sampler")]
    WeirdScaledSampler {
        rarity_value_mapper: String,
        noise: UnlocalizedName,
        input: DensityFunctionProvider,
    },
    #[serde(rename = "minecraft:y_clamped_gradient")]
    YClampedGradient {
        from_y: f64,
        to_y: f64,
        from_value: f64,
        to_value: f64,
    },
}


#[derive(Serialize, Deserialize)]
pub struct Spline {
    coordinate: DensityFunctionProvider,
    points: Vec<SplinePoint>,
}

#[derive(Serialize, Deserialize)]
pub struct SplinePoint {
    location: f64,
    value: SplineValue,
    derivative: f64,
}

#[derive(Serialize, Deserialize)]
pub enum SplineValue {
    Constant(f64),
    Spline(Spline),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerrainShaperSplineType {
    Offset,
    Factor,
    Jaggedness,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RarityValueType {
    Type1,
    Type2,
}
