use std::convert::{AsMut, AsRef};

#[derive(Clone)]
pub struct CompactStateBuffer<T> {
    inner: T,
    cursor: usize,
    bits: usize,
}

impl<'a> CompactStateBuffer<&'a Vec<i64>> {
    pub fn new(vec: &'a Vec<i64>, bits: usize) -> Self {
        CompactStateBuffer {
            inner: vec,
            cursor: 0,
            bits,
        }
    }
}

impl<'a> CompactStateBuffer<&'a mut Vec<i64>> {
    pub fn new(vec: &'a mut Vec<i64>, bits: usize) -> Self {
        CompactStateBuffer {
            inner: vec,
            cursor: 0,
            bits,
        }
    }
}

impl<T> CompactStateBuffer<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }

    fn advance_cursor(cursor: &mut usize, bit_index: usize, bits: usize) {
        if bit_index + 2 * bits > 64 {
            *cursor += 64 - bit_index;
        } else {
            *cursor += bits;
        }
    }

    pub fn skip(&mut self, entries: usize) {
        for _ in 0 .. entries {
            let bit_index = self.cursor % 64;
            Self::advance_cursor(&mut self.cursor, bit_index, self.bits);
        }
    }

    fn write_index_to(dest: &mut [i64], cursor: &mut usize, index: usize, bits: usize) -> bool {
        let long_index = *cursor / 64;
        let bit_index = *cursor % 64;

        debug_assert!(
            index < (1 << bits),
            "Index must be able to fit in allocated bits"
        );

        let insertion = (index as i64) << bit_index;
        match dest.get_mut(long_index) {
            Some(long) => *long |= insertion,
            None => return false,
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
    pub fn read_index(&mut self) -> Option<usize> {
        let slice = self.inner.as_ref();
        let long_index = self.cursor / 64;
        let bit_index = self.cursor % 64;

        let index = (*slice.get(long_index)? as u64 >> bit_index) & ((1u64 << self.bits) - 1);

        Self::advance_cursor(&mut self.cursor, bit_index, self.bits);
        Some(index as usize)
    }

    pub fn peak_index(&self) -> Option<usize> {
        let slice = self.inner.as_ref();
        let long_index = self.cursor / 64;
        let bit_index = self.cursor % 64;

        let index = (*slice.get(long_index)? as u64 >> bit_index) & ((1u64 << self.bits) - 1);

        Some(index as usize)
    }
}

impl CompactStateBuffer<&mut [i64]> {
    pub fn write_index(&mut self, index: usize, bits: usize) -> bool {
        Self::write_index_to(self.inner, &mut self.cursor, index, self.bits)
    }
}

impl CompactStateBuffer<&mut Vec<i64>> {
    pub fn write_index(&mut self, index: usize) -> bool {
        Self::write_index_to_vec(&mut self.inner, &mut self.cursor, index, self.bits);
        true
    }
}

impl CompactStateBuffer<Vec<i64>> {
    pub fn write_index(&mut self, index: usize) -> bool {
        Self::write_index_to_vec(&mut self.inner, &mut self.cursor, index, self.bits);
        true
    }
}
