use std::convert::{AsMut, AsRef};

pub struct CompactStateBuffer<T> {
    inner: T,
    cursor: usize,
}

impl CompactStateBuffer<Vec<i64>> {
    pub const fn new() -> Self {
        CompactStateBuffer {
            inner: Vec::new(),
            cursor: 0,
        }
    }
}

impl<'a, T: AsRef<[i64]> + ?Sized> CompactStateBuffer<&'a T> {
    pub fn read_index(&mut self, bits: usize) -> Option<usize> {
        let slice = self.inner.as_ref();
        let long_index = self.cursor / 64;
        let bit_index = self.cursor % 64;

        let start = (*slice.get(long_index)?) >> bit_index & ((1 << bits) - 1);
        let end = if bit_index + bits > 64 {
            (*slice.get(long_index + 1)? & ((1 << bit_index + bits - 64) - 1)) << 64 - bit_index
        } else {
            0
        };

        self.cursor += bits;
        Some((start | end) as usize)
    }
}

impl<'a, T: AsRef<[i64]> + ?Sized> From<&'a T> for CompactStateBuffer<&'a T> {
    fn from(inner: &'a T) -> Self {
        CompactStateBuffer { inner, cursor: 0 }
    }
}

impl<'a, T: AsRef<[i64]> + AsMut<[i64]> + ?Sized> From<&'a mut T>
    for CompactStateBuffer<&'a mut T>
{
    fn from(inner: &'a mut T) -> Self {
        CompactStateBuffer { inner, cursor: 0 }
    }
}
