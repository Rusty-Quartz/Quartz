use std::u128;
use std::fmt;

use rand::prelude::*;

const VERSION_VARIANT_MASK: u128 = 0xFFFFFFFF_FFFF_0FFF_3FFF_FFFFFFFFFFFF;
const VERSION_4_VARIANT_1: u128  = 0x00000000_0000_4000_8000_000000000000;

/// Represents a 128-bit (16-byte) universally unique identifier (UUID). This struct only
/// supports version 4, variant 1 UUIDs (IE randomly generated UUIDs).
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid(u128);

impl Uuid {
    /// Apply the correct version and variant to the UUID.
    #[inline(always)]
    fn correct_version(uuid: u128) -> u128 {
        (uuid & VERSION_VARIANT_MASK) | VERSION_4_VARIANT_1
    }

    /// Create a random UUID. Since the UUID must contain version and variant info, only 122
    /// out of the 128 bits are random.
    pub fn random() -> Self {
        Uuid(Self::correct_version(rand::thread_rng().gen()))
    }

    /// Converts the given bytes into a UUID and applies the correct version and variant information.
    /// This function will return an error if the given slice is not 16 bytes long.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 16 {
            return Err("Expected 16 bytes.");
        }

        let mut inner: u128 = 0;
        for i in 0..16 {
            inner |= bytes[i] as u128;
            inner <<= 8;
        }

        Ok(Uuid(Self::correct_version(inner)))
    }

    /// Converts the given string into a UUID and applies the correct version and variant information.
    /// This function will accept strings with or without dashes. If the string with dashes removed is not
    /// 32 hex characters long, then an error is returned. If the hex is invalid, then an error is also
    /// returned.
    pub fn from_string(string: &str) -> Result<Self, &'static str> {
        let raw = string.to_owned().replace("-", "");
        if raw.len() != 32 {
            return Err("Expected condensed string to have length 32.");
        }

        match u128::from_str_radix(&raw, 16) {
            Ok(inner) => Ok(Uuid(Self::correct_version(inner))),
            Err(_) => Err("Invalid UUID string.")
        }
    }

    /// Returns the inner u128 composing this UUID.
    pub fn as_u128(&self) -> u128 {
        self.0
    }
}

impl From<u128> for Uuid {
    fn from(inner: u128) -> Self {
        Uuid(inner)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            self.0 >> 96,
            (self.0 >> 80) & 0xFFFF,
            (self.0 >> 64) & 0xFFFF,
            (self.0 >> 48) & 0xFFFF,
            self.0 & 0xFFFFFFFFFFFF
        )
    }
}