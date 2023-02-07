use std::ops::{Mul, Sub};

use quartz_util::math::{binary_search, LerpExt};

use crate::density_function::{DensityFunctionContextWrapper, DensityFunctionRef};

pub enum SplineValue<C: Coordinate> {
    Constant(f32),
    Spline {
        coordinate: C,
        locations: Vec<f32>,
        values: Vec<SplineValue<C>>,
        derivatives: Vec<f32>,
    },
}

impl<C: Coordinate + Clone> Clone for SplineValue<C> {
    fn clone(&self) -> Self {
        match self {
            Self::Constant(c) => SplineValue::Constant(*c),
            Self::Spline {
                coordinate,
                locations,
                values,
                derivatives,
            } => SplineValue::Spline {
                coordinate: coordinate.clone(),
                locations: locations.clone(),
                values: values.clone(),
                derivatives: derivatives.clone(),
            },
        }
    }
}

impl<C: Coordinate> SplineValue<C> {
    pub fn apply(&self, point: &C::Point<'_>) -> f32 {
        match self {
            SplineValue::Constant(val) => *val,
            SplineValue::Spline {
                coordinate,
                locations,
                values,
                derivatives,
            } => {
                let raw_val = coordinate.apply(point);

                let i = binary_search(0, locations.len() as i32, |ix| {
                    raw_val < locations[ix as usize]
                }) - 1;

                let last_index = locations.len() - 1;

                if i < 0 {
                    values[0].apply(point) + derivatives[0] * (raw_val - locations[0])
                } else if i == last_index as i32 {
                    values[last_index].apply(point)
                        + derivatives[last_index] * (raw_val - locations[last_index])
                } else {
                    let i = i as usize;
                    let location = locations[i];
                    let next_location = locations[i + 1];
                    let raw_derivative = (raw_val - location) / (next_location - location);

                    let derivative = derivatives[i];
                    let next_derivative = derivatives[i + 1];

                    let val = values[i].apply(point);
                    let next_val = values[i + 1].apply(point);

                    let p = derivative * (next_location - location) - (next_val - val);
                    let q = -next_derivative * (next_location - location) + (next_val - val);

                    LerpExt::lerp(raw_derivative, val, next_val)
                        + raw_derivative
                            * (1.0 - raw_derivative)
                            * LerpExt::lerp(raw_derivative, p, q)
                }
            }
        }
    }
}

pub trait Coordinate {
    type Point<'a>;

    fn apply(&self, point: &Self::Point<'_>) -> f32;
}


#[derive(Clone, Copy)]
pub enum TerrainCoordinate {
    Continents,
    Erosion,
    Weirdness,
    Ridges,
}

impl Coordinate for TerrainCoordinate {
    type Point<'a> = TerrainPoint;

    fn apply(&self, point: &Self::Point<'_>) -> f32 {
        match self {
            Self::Continents => point.continents,
            Self::Erosion => point.erosion,
            Self::Weirdness => point.weirdness,
            Self::Ridges => point.ridges,
        }
    }
}

#[derive(Clone, Copy)]
pub struct TerrainPoint {
    continents: f32,
    erosion: f32,
    ridges: f32,
    weirdness: f32,
}


pub struct CustomCoordinate(DensityFunctionRef);

impl Coordinate for CustomCoordinate {
    type Point<'a> = DensityFunctionContextWrapper<'a>;

    fn apply(&self, point: &Self::Point<'_>) -> f32 {
        self.0.calculate(point) as f32
    }
}

pub struct SplineBuilder<C: Coordinate + Clone> {
    coordinate: C,
    value_transformer: fn(f32) -> f32,
    locations: Vec<f32>,
    values: Vec<SplineValue<C>>,
    derivatives: Vec<f32>,
}

impl<C: Coordinate + Clone> SplineBuilder<C> {
    fn new(coordinate: C) -> SplineBuilder<C> {
        Self::new_with_value_transformer(coordinate, identity_value_transformer)
    }

    fn new_with_value_transformer(
        coordinate: C,
        value_transformer: fn(f32) -> f32,
    ) -> SplineBuilder<C> {
        Self {
            coordinate,
            value_transformer,
            locations: Vec::new(),
            values: Vec::new(),
            derivatives: Vec::new(),
        }
    }

    pub fn add_const_point(self, location: f32, value: f32, derivative: f32) -> Self {
        let transformed_val = (self.value_transformer)(value);
        self.add_spline_point(location, SplineValue::Constant(transformed_val), derivative)
    }

    pub fn add_spline_point(
        mut self,
        location: f32,
        value: SplineValue<C>,
        derivative: f32,
    ) -> Self {
        if !self.locations.is_empty() && location < *self.locations.last().unwrap() {
            panic!("Please register Spline points in ascending order")
        } else {
            self.locations.push(location);
            self.values.push(value);
            self.derivatives.push(derivative);
        }
        self
    }

    pub fn build(self) -> SplineValue<C> {
        SplineValue::Spline {
            // This can be expensive if C is CustomCoordinate as density functions are uh, not cheap to clone
            // but I don't think SplineBuilder is ever used for CustomCoordinates and this should be done at startup anyway
            // if this ends up being a performance hole I'll switch over to a consuming build which will be cheaper
            coordinate: self.coordinate,
            // These technically create new allocations for their vecs but as with above, this should mostly
            // be done at startup and allocating 3 empty vecs is not too expensive for cold code
            locations: self.locations,
            values: self.values,
            derivatives: self.derivatives,
        }
    }
}

fn identity_value_transformer(val: f32) -> f32 {
    val
}


pub enum SplineType {
    Offset,
    Factor,
    Jaggedness,
}

pub struct TerrainShaper {
    offset_sampler: SplineValue<TerrainCoordinate>,
    factor_sampler: SplineValue<TerrainCoordinate>,
    jaggedness_sampler: SplineValue<TerrainCoordinate>,
}

impl TerrainShaper {
    fn get_amplified_offset(offset: f32) -> f32 {
        if offset < 0.0 {
            offset
        } else {
            offset * 2.0
        }
    }

    fn get_amplified_factor(factor: f32) -> f32 {
        1.25 - 6.25 / (factor + 5.0)
    }

    fn get_amplified_jaggedness(jaggedness: f32) -> f32 {
        jaggedness * 2.0
    }

    fn slope(y1: f32, y2: f32, x1: f32, x2: f32) -> f32 {
        (y2 - y1) / (x2 - x1)
    }

    pub fn factor(&self, point: &TerrainPoint) -> f32 {
        self.factor_sampler.apply(point)
    }

    pub fn jaggedness(&self, point: &TerrainPoint) -> f32 {
        self.jaggedness_sampler.apply(point)
    }

    pub fn offset(&self, point: &TerrainPoint) -> f32 {
        self.offset_sampler.apply(point) - 0.50375
    }

    pub fn peaks_and_valleys(weirdness: f32) -> f32 {
        weirdness
            .abs()
            .sub(0.6666667)
            .abs()
            .sub(0.33333334)
            .mul(-3.0)
    }

    pub fn make_point(continents: f32, erosion: f32, weirdness: f32) -> TerrainPoint {
        TerrainPoint {
            continents,
            erosion,
            ridges: Self::peaks_and_valleys(weirdness),
            weirdness,
        }
    }

    fn mountain_continentalness(f: f32, g: f32, h: f32) -> f32 {
        let i = 1.17;
        let j = 0.46082947;
        let k = 1.0 - (1.0 - g) * 0.5;
        let l = 0.5 * (1.0 - g);
        let m = (f + i) * j;
        let n = m * k - l;
        if f < h {
            n.max(-0.2222)
        } else {
            n.max(0.0)
        }
    }

    fn calculate_mountain_ridge_zero_continentalness_point(f: f32) -> f32 {
        let g = 1.17;
        let h = 0.46082947;
        let i = 1.0 - (1.0 - f) * 0.5;
        let j = 0.5 * (1.0 - f);
        j / (h * i) - g
    }

    fn ridge_spline(
        value_transformer: fn(f32) -> f32,
        val1: f32,
        val2: f32,
        val3: f32,
        val4: f32,
        val5: f32,
        val6: f32,
    ) -> SplineValue<TerrainCoordinate> {
        let f = val6.max(0.5 * (val2 - val1));
        let g = 5.0 * (val3 - val2);


        SplineBuilder::new_with_value_transformer(TerrainCoordinate::Ridges, value_transformer)
            .add_const_point(-1.0, val1, f)
            .add_const_point(-0.4, val2, f.min(g))
            .add_const_point(0.0, val3, g)
            .add_const_point(0.4, val4, 2.0 * (val4 - val3))
            .add_const_point(1.0, val5, 0.7 * (val5 - val4))
            .build()
    }

    fn build_mountain_ridge_spline_with_points(
        value: f32,
        b1: bool,
        value_transformer: fn(f32) -> f32,
    ) -> SplineValue<TerrainCoordinate> {
        let mut builder =
            SplineBuilder::new_with_value_transformer(TerrainCoordinate::Ridges, value_transformer);
        let f = -0.7;
        let g = -1.0;
        let h = Self::mountain_continentalness(-1.0, value, -0.7);
        let i = 1.0;
        let j = Self::mountain_continentalness(1.0, value, -0.7);
        let k = Self::calculate_mountain_ridge_zero_continentalness_point(value);
        let l = -0.65;

        if l < k && k < i {
            let m = Self::mountain_continentalness(l, value, f);
            let n = -0.75;
            let o = Self::mountain_continentalness(n, value, f);
            let p = Self::slope(h, o, g, n);
            builder = builder
                .add_const_point(g, h, p)
                .add_const_point(n, o, 0.0)
                .add_const_point(l, m, 0.0);
            let q = Self::mountain_continentalness(k, value, f);
            let r = Self::slope(q, j, k, i);
            let s = 0.01;
            builder = builder
                .add_const_point(k - s, q, 0.0)
                .add_const_point(k, q, r)
                .add_const_point(i, j, r);
        } else {
            let m = Self::slope(h, j, g, i);
            if b1 {
                builder = builder.add_const_point(g, h.max(0.2), 0.0).add_const_point(
                    0.0,
                    LerpExt::lerp(0.5, h, j),
                    m,
                );
            } else {
                builder = builder.add_const_point(g, h, m);
            }
        }
        builder.build()
    }

    fn get_erosion_factor(
        float: f32,
        b1: bool,
        value_transformer: fn(f32) -> f32,
    ) -> SplineValue<TerrainCoordinate> {
        let spline_1 = SplineBuilder::new_with_value_transformer(
            TerrainCoordinate::Weirdness,
            value_transformer,
        )
        .add_const_point(-0.2, 6.3, 0.0)
        .add_const_point(0.2, float, 0.0)
        .build();

        let mut builder = SplineBuilder::new_with_value_transformer(
            TerrainCoordinate::Erosion,
            value_transformer,
        );
        builder = builder
            .add_spline_point(-0.6, spline_1.clone(), 0.0)
            .add_spline_point(
                -0.5,
                SplineBuilder::new_with_value_transformer(
                    TerrainCoordinate::Weirdness,
                    value_transformer,
                )
                .add_const_point(-0.05, 6.3, 0.0)
                .add_const_point(0.05, 2.67, 0.0)
                .build(),
                0.0,
            )
            .add_spline_point(-0.35, spline_1.clone(), 0.0)
            .add_spline_point(-0.25, spline_1.clone(), 0.0)
            .add_spline_point(
                -0.1,
                SplineBuilder::new_with_value_transformer(
                    TerrainCoordinate::Weirdness,
                    value_transformer,
                )
                .add_const_point(-0.05, 2.67, 0.0)
                .add_const_point(0.05, 6.3, 0.0)
                .build(),
                0.0,
            )
            .add_spline_point(0.03, spline_1.clone(), 0.0);

        if b1 {
            let spline_2 = SplineBuilder::new_with_value_transformer(
                TerrainCoordinate::Weirdness,
                value_transformer,
            )
            .add_const_point(0.0, float, 0.0)
            .add_const_point(0.1, 0.625, 0.0)
            .build();
            let spline_3 = SplineBuilder::new_with_value_transformer(
                TerrainCoordinate::Ridges,
                value_transformer,
            )
            .add_const_point(-0.9, float, 0.0)
            .add_spline_point(-0.69, spline_2, 0.0)
            .build();

            builder = builder
                .add_const_point(0.35, float, 0.0)
                .add_spline_point(0.45, spline_3, 0.0);
        } else {
            let spline_2 = SplineBuilder::new_with_value_transformer(
                TerrainCoordinate::Ridges,
                value_transformer,
            )
            .add_spline_point(-0.7, spline_1.clone(), 0.0)
            .add_const_point(-0.15, 1.37, 0.0)
            .build();

            let spline_3 = SplineBuilder::new_with_value_transformer(
                TerrainCoordinate::Ridges,
                value_transformer,
            )
            .add_spline_point(0.45, spline_1, 0.0)
            .add_const_point(0.7, 1.56, 0.0)
            .build();

            builder = builder
                .add_spline_point(0.05, spline_3.clone(), 0.0)
                .add_spline_point(0.4, spline_3, 0.0)
                .add_spline_point(0.45, spline_2.clone(), 0.0)
                .add_spline_point(0.55, spline_2, 0.0)
                .add_const_point(0.58, float, 0.0);
        }

        builder.build()
    }

    fn build_weirdness_jaggedness_spline(
        value: f32,
        value_transformer: fn(f32) -> f32,
    ) -> SplineValue<TerrainCoordinate> {
        SplineBuilder::new_with_value_transformer(TerrainCoordinate::Weirdness, value_transformer)
            .add_const_point(-0.01, 0.63 * value, 0.0)
            .add_const_point(0.01, 0.3 * value, 0.0)
            .build()
    }

    fn build_ridge_jaggedness_spline(
        val: f32,
        val2: f32,
        value_transformer: fn(f32) -> f32,
    ) -> SplineValue<TerrainCoordinate> {
        let f = Self::peaks_and_valleys(0.4);
        let g = Self::peaks_and_valleys(0.56666666);
        let h = (f + g) / 2.0;
        let mut builder =
            SplineBuilder::new_with_value_transformer(TerrainCoordinate::Ridges, value_transformer)
                .add_const_point(f, 0.0, 0.0);

        if val2 > 0.0 {
            builder = builder.add_spline_point(
                h,
                Self::build_weirdness_jaggedness_spline(val2, value_transformer),
                0.0,
            );
        } else {
            builder = builder.add_const_point(h, 0.0, 0.0);
        }

        if val > 0.0 {
            builder = builder.add_spline_point(
                1.0,
                Self::build_weirdness_jaggedness_spline(val, value_transformer),
                0.0,
            );
        } else {
            builder = builder.add_const_point(1.0, 0.0, 0.0);
        }

        builder.build()
    }

    fn build_erosion_jaggedness_spline(
        f: f32,
        g: f32,
        h: f32,
        i: f32,
        value_transformer: fn(f32) -> f32,
    ) -> SplineValue<TerrainCoordinate> {
        let spline_1 = Self::build_ridge_jaggedness_spline(f, h, value_transformer);
        let spline_2 = Self::build_ridge_jaggedness_spline(g, i, value_transformer);

        SplineBuilder::new_with_value_transformer(TerrainCoordinate::Erosion, value_transformer)
            .add_spline_point(-1.0, spline_1, 0.0)
            .add_spline_point(-0.78, spline_2.clone(), 0.0)
            .add_spline_point(-0.5775, spline_2, 0.0)
            .add_const_point(-0.375, 0.0, 0.0)
            .build()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_erosion_offset_spline(
        val: f32,
        val2: f32,
        val3: f32,
        val4: f32,
        val5: f32,
        val6: f32,
        b1: bool,
        b2: bool,
        value_transformer: fn(f32) -> f32,
    ) -> SplineValue<TerrainCoordinate> {
        let spline_1 = Self::build_mountain_ridge_spline_with_points(
            LerpExt::lerp(val4, 0.6, 1.5),
            b2,
            value_transformer,
        );
        let spline_2 = Self::build_mountain_ridge_spline_with_points(
            LerpExt::lerp(val4, 0.6, 1.0),
            b2,
            value_transformer,
        );
        let spline_3 = Self::build_mountain_ridge_spline_with_points(val4, b2, value_transformer);
        let spline_4 = Self::ridge_spline(
            value_transformer,
            val - 0.15,
            val4 * 0.5,
            LerpExt::lerp(0.5, 0.5, 0.5) * val4,
            0.5 * val4,
            0.6 * val4,
            0.5,
        );
        let spline_5 = Self::ridge_spline(
            value_transformer,
            val,
            val5 * val4,
            val2 * val4,
            0.5 * val4,
            0.6 * val4,
            0.5,
        );
        let spline_6 = Self::ridge_spline(value_transformer, val, val5, val5, val2, val3, 0.5);
        let spline_7 = Self::ridge_spline(value_transformer, val, val5, val5, val2, val3, 0.5);
        let spline_8 =
            SplineBuilder::new_with_value_transformer(TerrainCoordinate::Ridges, value_transformer)
                .add_const_point(-1.0, val, 0.0)
                .add_spline_point(-0.4, spline_6.clone(), 0.0)
                .add_const_point(0.0, val3 + 0.07, 0.0)
                .build();
        let spline_9 = Self::ridge_spline(value_transformer, -0.02, val6, val6, val2, val3, 0.0);

        let mut builder = SplineBuilder::new_with_value_transformer(
            TerrainCoordinate::Erosion,
            value_transformer,
        )
        .add_spline_point(-0.85, spline_1, 0.0)
        .add_spline_point(-0.7, spline_2, 0.0)
        .add_spline_point(-0.4, spline_3, 0.0)
        .add_spline_point(-0.35, spline_4, 0.0)
        .add_spline_point(-0.1, spline_5, 0.0)
        .add_spline_point(0.2, spline_6, 0.0);

        if b1 {
            builder = builder
                .add_spline_point(0.4, spline_7.clone(), 0.0)
                .add_spline_point(0.45, spline_8.clone(), 0.0)
                .add_spline_point(0.55, spline_8, 0.0)
                .add_spline_point(0.58, spline_7, 0.0);
        }

        builder.add_spline_point(0.7, spline_9, 0.0).build()
    }

    pub fn overworld(transform: bool) -> Self {
        let offset = if transform {
            Self::get_amplified_offset
        } else {
            identity_value_transformer
        };
        let factor = if transform {
            Self::get_amplified_factor
        } else {
            identity_value_transformer
        };
        let jaggedness = if transform {
            Self::get_amplified_jaggedness
        } else {
            identity_value_transformer
        };

        let erosion_offset_spline_1 = Self::build_erosion_offset_spline(
            -0.15, 0.0, 0.0, 0.1, 0.0, -0.03, false, false, offset,
        );
        let erosion_offset_spline_2 = Self::build_erosion_offset_spline(
            -0.1, 0.03, 0.1, 0.1, 0.01, -0.03, false, false, offset,
        );
        let erosion_offset_spline_3 = Self::build_erosion_offset_spline(
            -0.1, 0.03, 0.1, 0.7, 0.01, -0.03, true, true, offset,
        );
        let erosion_offset_spline_4 = Self::build_erosion_offset_spline(
            -0.05, 0.03, 0.1, 1.0, 0.01, 0.01, true, true, offset,
        );

        let offset_sampler =
            SplineBuilder::new_with_value_transformer(TerrainCoordinate::Continents, offset)
                .add_const_point(-1.01, 0.044, 0.0)
                .add_const_point(-1.02, -0.2222, 0.0)
                .add_const_point(-0.51, -0.2222, 0.0)
                .add_const_point(-0.44, -0.12, 0.0)
                .add_const_point(-0.18, -0.12, 0.0)
                .add_spline_point(-0.16, erosion_offset_spline_1.clone(), 0.0)
                .add_spline_point(-0.15, erosion_offset_spline_1, 0.0)
                .add_spline_point(-0.1, erosion_offset_spline_2, 0.0)
                .add_spline_point(0.25, erosion_offset_spline_3, 0.0)
                .add_spline_point(1.0, erosion_offset_spline_4, 0.0)
                .build();

        let factor_sampler = SplineBuilder::new(TerrainCoordinate::Continents)
            .add_const_point(-0.19, 3.95, 0.0)
            .add_spline_point(
                -0.15,
                Self::get_erosion_factor(6.25, true, identity_value_transformer),
                0.0,
            )
            .add_spline_point(-0.1, Self::get_erosion_factor(5.47, true, factor), 0.0)
            .add_spline_point(0.03, Self::get_erosion_factor(5.08, true, factor), 0.0)
            .add_spline_point(0.06, Self::get_erosion_factor(4.69, false, factor), 0.0)
            .build();

        let jaggedness_sampler =
            SplineBuilder::new_with_value_transformer(TerrainCoordinate::Continents, jaggedness)
                .add_const_point(-0.11, 0.0, 0.0)
                .add_spline_point(
                    0.03,
                    Self::build_erosion_jaggedness_spline(1.0, 0.5, 0.0, 0.0, jaggedness),
                    0.0,
                )
                .add_spline_point(
                    0.65,
                    Self::build_erosion_jaggedness_spline(1.0, 1.0, 1.0, 0.0, jaggedness),
                    0.0,
                )
                .build();

        TerrainShaper {
            offset_sampler,
            factor_sampler,
            jaggedness_sampler,
        }
    }
}
