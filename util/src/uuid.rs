use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use std::u128;

use rand::prelude::*;

const VERSION_VARIANT_MASK: u128 = 0xFFFFFFFF_FFFF_0FFF_3FFF_FFFFFFFFFFFF;
const VERSION_4_VARIANT_1: u128 = 0x00000000_0000_4000_8000_000000000000;

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
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Uuid;
    /// println!("{}", Uuid::random());
    /// // Sample output: 7919a79b-b256-4782-bf36-13990ca65bb7
    /// ```
    pub fn random() -> Self {
        Uuid(Self::correct_version(rand::thread_rng().gen()))
    }

    /// Converts the given bytes (big-endian) into a UUID and applies the correct version and variant information.
    /// This function will return an error if the given slice is not 16 bytes long.
    ///
    /// # Panics
    ///
    /// Panics if the input slice is not 16 bytes long.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Uuid;
    /// let uuid = Uuid::from_bytes_be(&[121, 25, 167, 155, 178, 86, 71, 130, 191, 54, 19, 153, 12, 166, 91, 183]);
    /// assert_eq!(uuid.as_u128(), 0x7919a79bb2564782bf3613990ca65bb7_u128);
    /// ```
    pub fn from_bytes_be(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), 16, "Expected 16 bytes.");

        let mut inner: u128 = 0;
        for i in 0..15 {
            inner |= bytes[i] as u128;
            inner <<= 8;
        }
        inner |= bytes[15] as u128;

        Uuid(Self::correct_version(inner))
    }

    /// Converts the given bytes (little-endian) into a UUID and applies the correct version and variant information.
    /// This function will return an error if the given slice is not 16 bytes long.
    ///
    /// # Panics
    ///
    /// Panics if the input slice is not 16 bytes long.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Uuid;
    /// let uuid = Uuid::from_bytes_le(&[183, 91, 166, 12, 153, 19, 54, 191, 130, 71, 86, 178, 155, 167, 25, 121]);
    /// assert_eq!(uuid.as_u128(), 0x7919a79bb2564782bf3613990ca65bb7_u128);
    /// ```
    pub fn from_bytes_le(bytes: &[u8]) -> Self {
        assert_eq!(bytes.len(), 16, "Expected 16 bytes.");

        let mut inner: u128 = 0;
        for i in 1..16 {
            inner |= bytes[16 - i] as u128;
            inner <<= 8;
        }
        inner |= bytes[0] as u128;

        Uuid(Self::correct_version(inner))
    }

    /// Returns the most significant 64 bits of this UUID's 128-bit value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Uuid;
    /// let uuid = Uuid::from_str("7919a79b-b256-4782-bf36-13990ca65bb7").unwrap();
    /// assert_eq!(uuid.most_significant_bits(), 0x7919a79bb2564782_u64);
    /// ```
    pub fn most_significant_bits(&self) -> u64 {
        (self.0 >> 64) as u64
    }

    /// Returns the least significant 64 bits of this UUID of this UUID's 128-bit value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Uuid;
    /// let uuid = Uuid::from_str("7919a79b-b256-4782-bf36-13990ca65bb7").unwrap();
    /// assert_eq!(uuid.least_significant_bits(), 0xbf3613990ca65bb7_u64);
    /// ```
    pub fn least_significant_bits(&self) -> u64 {
        self.0 as u64
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

impl FromStr for Uuid {
    type Err = &'static str;

    /// Converts the given string into a UUID and applies the correct version and variant information.
    ///
    /// This function will accept strings with or without dashes. If the string with dashes removed is not
    /// 32 hex characters long, then an error is returned. If the hex is invalid, then an error is also
    /// returned.
    ///
    /// # Examples
    ///
    /// ```
    /// # use util::Uuid;
    /// let uuid = Uuid::from_str("7919a79b-b256-4782-bf36-13990ca65bb7").unwrap();
    /// assert_eq!(uuid.as_u128(), 0x7919a79bb2564782bf3613990ca65bb7_u128);
    ///
    /// assert!(Uuid::from_str("invalid").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let raw = s.to_owned().replace("-", "");
        if raw.len() != 32 {
            return Err("Expected condensed string to have length 32.");
        }

        match u128::from_str_radix(&raw, 16) {
            Ok(inner) => Ok(Uuid(Self::correct_version(inner))),
            Err(_) => Err("Invalid UUID string."),
        }
    }
}

impl Display for Uuid {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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
