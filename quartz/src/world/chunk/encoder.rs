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

impl<T> CompactStateBuffer<T> {
    fn advance_cursor(cursor: &mut usize, bit_index: usize, bits: usize) {
        if bit_index + 2 * bits > 64 {
            *cursor += 64 - bit_index;
        } else {
            *cursor += bits;
        }
    }

    fn write_index_to(dest: &mut [i64], cursor: &mut usize, index: usize, bits: usize) -> bool {
        let long_index = *cursor / 64;
        let bit_index = *cursor % 64;

        debug_assert!(index < (1 << bits), "Index must be able to fit in allocated bits");

        let insertion = (index as i64) << bit_index;
        match dest.get_mut(long_index) {
            Some(long) => *long |= insertion,
            None => return false
        }

        Self::advance_cursor(cursor, bit_index, bits);
        true
    }

    fn write_index_to_vec(dest: &mut Vec<i64>, cursor: &mut usize, index: usize, bits: usize) {
        if *cursor / 64 >= dest.len() {
            dest.push(0);
        }
        debug_assert!(
            Self::write_index_to(&mut *dest, cursor, index, bits),
            "Failed to expand vec to required capacity in compact state buffer."
        );
    }
}

impl<T: AsRef<[i64]>> CompactStateBuffer<T> {
    pub fn read_index(&mut self, bits: usize) -> Option<usize> {
        let slice = self.inner.as_ref();
        let long_index = self.cursor / 64;
        let bit_index = self.cursor % 64;

        let index = (*slice.get(long_index)? as u64 >> bit_index) & ((1u64 << bits) - 1);

        Self::advance_cursor(&mut self.cursor, bit_index, bits);
        Some(index as usize)
    }
}

impl CompactStateBuffer<&mut [i64]> {
    pub fn write_index(&mut self, index: usize, bits: usize) -> bool {
        Self::write_index_to(self.inner, &mut self.cursor, index, bits)
    }
}

impl CompactStateBuffer<&mut Vec<i64>> {
    pub fn write_index(&mut self, index: usize, bits: usize) -> bool {
        Self::write_index_to_vec(&mut self.inner, &mut self.cursor, index, bits);
        true
    }
}

impl CompactStateBuffer<Vec<i64>> {
    pub fn write_index(&mut self, index: usize, bits: usize) -> bool {
        Self::write_index_to_vec(&mut self.inner, &mut self.cursor, index, bits);
        true
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
