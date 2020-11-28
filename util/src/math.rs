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
/// # use util::math::fast_log2_64;
/// for i in 0..64 {
///     assert_eq!(fast_log2_64(1 << i), i as u64);
/// }
///
/// assert_eq!(fast_log2_64(651854213), 29); // Exact: 29.2799741
/// assert_eq!(fast_log2_64(0), 63);
/// ```
pub const fn fast_log2_64(mut value: u64) -> u64 {
    value |= value >> 1;
    value |= value >> 2;
    value |= value >> 4;
    value |= value >> 8;
    value |= value >> 16;
    value |= value >> 32;

    return LOG2_TABLE_64[((value - (value >> 1))
        .overflowing_mul(0x07EDD5E59A4E28C2u64)
        .0
        >> 58) as usize];
}

/// Computes the inverse square root of the given floating point number. The approximation produced by
/// this algorithm is fairly rough, but generally speaking the relative error is less than plus or minus
/// one-tenth of one percent.
#[inline]
pub fn fast_inv_sqrt64(mut value: f64) -> f64 {
    let i: u64;
    let x: f64;

    x = value * 0.5;
    i = 0x5FE6EB50C7B537A9 - (value.to_bits() >> 1);
    value = f64::from_bits(i);

    // This constant is a short-cut for another iteration of Newton's method. It is the optimal number to
    // reduce the mean squared relative error of this algorithm.
    1.0009632777831923 * value * (1.5 - (x * value * value))
}
