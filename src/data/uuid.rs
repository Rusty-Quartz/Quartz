use std::u128;
use std::fmt;

use rand::prelude::*;

#[repr(transparent)]
#[derive(Copy, Clone)]
pub struct Uuid(u128);

impl Uuid {
    pub fn random() -> Self {
        Uuid(rand::thread_rng().gen())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 16 {
            return Err("Expected 16 bytes.");
        }

        let mut inner: u128 = 0;
        for i in 0..16 {
            inner |= bytes[i] as u128;
            inner <<= 8;
        }

        Ok(Uuid(inner))
    }

    pub fn from_string(string: &str) -> Result<Self, &'static str> {
        let raw = string.to_owned().replace("-", "");
        if raw.len() != 32 {
            return Err("Expected condensed string to have length 32.");
        }

        match u128::from_str_radix(&raw, 16) {
            Ok(inner) => Ok(Uuid(inner)),
            Err(_) => Err("Invalid UUID string.")
        }
    }

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
            f, "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            self.0 >> 96,
            (self.0 >> 80) & 0xFFFF,
            (self.0 >> 64) & 0xFFFF,
            (self.0 >> 48) & 0xFFFF,
            self.0 & 0xFFFFFFFFFFFF
        )
    }
}