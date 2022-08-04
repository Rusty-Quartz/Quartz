use quartz_util::math::unsigned_shr_i64;

/// Converts a i64 to a u128 to be used as a random seed
pub fn i64_seed_to_u128_seed(long: i64) -> (i64, i64) {
    let low = long ^ 7640891576956012809;
    let high = low + -7046029254386353131;


    (mix_stafford_13(low), mix_stafford_13(high))
}

// I honestly have no idea wtf this is doing
// but its needed if we want to have seed parity w/ minecraft
pub fn mix_stafford_13(mut long: i64) -> i64 {
    long = (long ^ unsigned_shr_i64(long, 30)).wrapping_mul(-4658895280553007687);
    long = (long ^ unsigned_shr_i64(long, 27)).wrapping_mul(-7723592293110705685);
    long ^ unsigned_shr_i64(long, 31)
}

/// Returns a positional random seed
pub fn get_pos_seed(x: i32, y: i32, z: i32) -> i64 {
    let temp = ((x as i64).wrapping_mul(3129871)) ^ ((z as i64).wrapping_mul(116129781)) ^ y as i64;
    temp.wrapping_mul(temp)
        .wrapping_mul(42317861)
        .wrapping_add(temp.wrapping_mul(11))
        >> 16
}

/// Returns the md5 hash of the string
pub fn hash_string_md5(s: &str) -> (i64, i64) {
    use md5::{Digest, Md5};
    // I think this has full parity w/ minecraft
    // though I'm unsure cause I think javax's md5 uses 128 bit blocks
    let mut hasher = Md5::new();
    hasher.update(s);
    let result = hasher.finalize();
    (build_long(&result[..]), build_long(&result[8 ..]))
}

fn build_long(bytes: &[u8]) -> i64 {
    ((bytes[0] as i64) << 56
        | (bytes[1] as i64) << 48
        | (bytes[2] as i64) << 40
        | (bytes[3] as i64) << 32
        | (bytes[4] as i64) << 24
        | (bytes[5] as i64) << 16
        | (bytes[6] as i64) << 8
        | (bytes[7] as i64))
}


pub fn java_string_hash(str: &str) -> i32 {
    let mut h = 0_i32;

    for char in str.chars() {
        h = 31_i32.wrapping_mul(h).wrapping_add(char as i32);
    }

    h
}
