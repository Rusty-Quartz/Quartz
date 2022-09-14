use std::ops::{Add, BitXor, Div, Mul, Sub};

const LOG2_TABLE_64: [u64; 64] = [
    63, 0, 58, 1, 59, 47, 53, 2, 60, 39, 48, 27, 54, 33, 42, 3, 61, 51, 37, 40, 49, 18, 28, 20, 55,
    30, 34, 11, 43, 14, 22, 4, 62, 57, 46, 52, 38, 26, 32, 41, 50, 36, 17, 19, 29, 10, 13, 21, 56,
    45, 25, 31, 35, 16, 9, 12, 44, 24, 15, 8, 23, 7, 6, 5,
];

/// Computes the base-2 logarithm of a 64-bit value using a DeBruijn-like algorithm, flooring the result.
/// Note that an input of zero will result in an output of `63` rather than an error.
///
/// # Examples
///
/// ```
/// # use quartz_util::math::fast_log2_64;
/// for i in 0..64 {
///     assert_eq!(fast_log2_64(1 << i), i as u64);
/// }
///
/// assert_eq!(fast_log2_64(15), 3);
/// assert_eq!(fast_log2_64(17), 4);
/// assert_eq!(fast_log2_64(651854213), 29); // Exact: 29.2799741
/// assert_eq!(fast_log2_64(0), 63);
/// ```
#[inline]
pub const fn fast_log2_64(mut value: u64) -> u64 {
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;
    value |= value >> 32;

    LOG2_TABLE_64[((value - (value >> 1))
        .overflowing_mul(0x07EDD5E59A4E28C2u64)
        .0
        >> 58) as usize]
}

/// Computes the base-2 logarithm of a 64-bit value using a DeBruijn-like algorithm, ceiling the result.
/// Note that an input of zero will result in an output of `63` rather than an error.
///
/// # Examples
///
/// ```
/// # use quartz_util::math::fast_ceil_log2_64;
/// for i in 0..64 {
///     assert_eq!(fast_ceil_log2_64(1 << i), i as u64);
/// }
///
/// assert_eq!(fast_ceil_log2_64(15), 4);
/// assert_eq!(fast_ceil_log2_64(17), 5);
/// assert_eq!(fast_ceil_log2_64(651854213), 30); // Exact: 29.2799741
/// assert_eq!(fast_ceil_log2_64(0), 63);
/// ```
#[inline]
pub const fn fast_ceil_log2_64(value: u64) -> u64 {
    fast_log2_64(value.overflowing_shl(1).0.overflowing_sub(1).0)
}

/// Computes the inverse square root of the given floating point number. The approximation produced by
/// this algorithm is fairly rough, but generally speaking the relative error is less than plus or minus
/// one-tenth of one percent.
#[inline]
pub fn fast_inv_sqrt64(mut value: f64) -> f64 {
    let i: u64 = 0x5FE6EB50C7B537A9 - (value.to_bits() >> 1);
    let x: f64 = value * 0.5;

    value = f64::from_bits(i);

    // This constant is a short-cut for another iteration of Newton's method. It is the optimal number to
    // reduce the mean squared relative error of this algorithm.
    1.0009632777831923 * value * (1.5 - (x * value * value))
}

/// Preforms an unsigned bitshift right
pub const fn unsigned_shr(lhs: i32, rhs: i32) -> i32 {
    (lhs as u32 >> rhs) as i32
}

/// Preforms an unsigned bitshift right
pub const fn unsigned_shr_i64(lhs: i64, rhs: i64) -> i64 {
    (lhs as u64 >> rhs) as i64
}

/// Preforms a binary search delagating the test function to a supplied closure
pub fn binary_search(mut min: i32, max: i32, test: impl Fn(i32) -> bool) -> i32 {
    let mut i = max - min;

    while i > 0 {
        let j = i / 2;
        let k = min + j;
        if test(k) {
            i = j;
        } else {
            min = k + 1;
            i -= j + 1;
        }
    }

    min
}

/// Returns the dot product of the provided vector and the vector <x, y, z>
pub fn dot(gradient: [i32; 3], x: f64, y: f64, z: f64) -> f64 {
    gradient[0] as f64 * x + gradient[1] as f64 * y + gradient[2] as f64 * z
}

// I actually don't really know what this does
pub fn smooth_step(val: f64) -> f64 {
    val * val * val * (val * (val * 6.0 - 15.0) + 10.0)
}

/// Trait to provide methods to preform linear interpolation
pub trait LerpExt:
    Add<Self, Output = Self>
    + Div<Self, Output = Self>
    + Sub<Self, Output = Self>
    + Mul<Self, Output = Self>
    + PartialOrd
    + Sized
    + Copy
{
    /// The zero value of the type
    const ZERO: Self;
    /// The one value of the type
    const ONE: Self;

    /// Preforms linear interpolation
    fn lerp(delta: Self, start: Self, end: Self) -> Self {
        start + delta * (end - start)
    }

    /// Preforms linear interpolation with delta being clamped to [0.0, 1.0]
    fn clamped_lerp(delta: Self, start: Self, end: Self) -> Self {
        if delta < Self::ZERO {
            start
        } else if delta > Self::ONE {
            end
        } else {
            Self::lerp(delta, start, end)
        }
    }

    /// Maps an interpolated value in one range to a new range
    fn clamped_map(
        value: Self,
        from_start: Self,
        from_end: Self,
        to_start: Self,
        to_end: Self,
    ) -> Self {
        Self::clamped_lerp(
            Self::inverse_lerp(value, from_start, from_end),
            to_start,
            to_end,
        )
    }

    /// Takes a value and returns the coresponding delta value for use in lerp
    fn inverse_lerp(value: Self, start: Self, end: Self) -> Self {
        (value - start) / (end - start)
    }
}

impl LerpExt for f32 {
    const ONE: Self = 1.0;
    const ZERO: Self = 0.0;
}

impl LerpExt for f64 {
    const ONE: Self = 1.0;
    const ZERO: Self = 0.0;
}

/// Divides x by y rounding down to the nearest integer
pub const fn div_floor(x: i32, y: i32) -> i32 {
    let mut r = x / y;

    if (x ^ y) < 0 && (r * y != x) {
        r -= 1;
    }

    r
}
