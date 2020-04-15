use std::mem::transmute;
use std::str;
use std::fmt;

pub struct ByteBuffer {
    inner: Vec<u8>,
    cursor: usize
}

impl From<&[u8]> for ByteBuffer {
    fn from(bytes: &[u8]) -> Self {
        ByteBuffer {
            inner: Vec::from(bytes),
            cursor: 0
        }
    }
}

impl<Idx> std::ops::Index<Idx> for ByteBuffer
where
    Idx: std::slice::SliceIndex<[u8]>
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.inner[index]
    }
}

impl<Idx> std::ops::IndexMut<Idx> for ByteBuffer
where
    Idx: std::slice::SliceIndex<[u8]>
{
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.inner[index]
    }
}

impl fmt::Display for ByteBuffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:X?}", self.inner)
    }
}

impl ByteBuffer {
    pub fn new(initial_size: usize) -> ByteBuffer {
        ByteBuffer {
            inner: vec![0; initial_size],
            cursor: 0
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn ensure_capacity(&mut self, capacity: usize) {
        if capacity > self.inner.capacity() {
            self.inner.reserve_exact(capacity - self.inner.capacity());
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn inflate(&mut self) {
        unsafe {
            self.inner.set_len(self.inner.capacity());
        }
    }

    #[inline]
    pub fn ensure_size(&mut self, size: usize) {
        self.ensure_capacity(size);
        if self.inner.len() < size {
            unsafe {
                self.inner.set_len(size);
            }
        }
    }

    #[inline]
    pub fn resize(&mut self, size: usize) {
        self.ensure_capacity(size);
        if size < self.cursor {
            self.cursor = size;
        }
        unsafe {
            self.inner.set_len(size);
        }
    }

    #[inline]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    #[inline]
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    #[inline]
    pub fn remaining(&self) -> usize {
        self.inner.len() - self.cursor()
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
        self.cursor = 0;
    }

    #[inline]
    pub fn append_bytes(&mut self, bytes: &[u8]) {
        self.resize(self.cursor() + bytes.len());
        self.inner[self.cursor..self.cursor + bytes.len()].copy_from_slice(bytes);
    }

    #[inline]
    pub fn inner_mut(&mut self) -> &mut Vec<u8> {
        &mut self.inner
    }

    #[inline]
    pub fn read(&mut self) -> u8 {
        if self.cursor >= self.inner.len() {
            return 0;
        }

        let byte = self.inner[self.cursor];
        self.cursor += 1;
        byte
    }

    pub fn read_bytes(&mut self, dest: &mut Vec<u8>) {
        let len = dest.len();
        dest.copy_from_slice(&self.inner[self.cursor..self.cursor + len]);
        self.cursor += len;
    }

    #[inline]
    pub fn read_bool(&mut self) -> bool {
        self.read() != 0
    }

    #[inline]
    pub fn read_u8(&mut self) -> u8 {
        self.read()
    }

    #[inline]
    pub fn read_i8(&mut self) -> i8 {
        self.read() as i8
    }

    #[inline]
    pub fn read_u16(&mut self) -> u16 {
        if self.cursor + 1 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as u16) << 8 | (self.inner[self.cursor + 1] as u16);
        self.cursor += 2;
        result
    }

    #[inline]
    pub fn read_i16(&mut self) -> i16 {
        self.read_u16() as i16
    }

    #[inline]
    pub fn read_i32(&mut self) -> i32 {
        if self.cursor + 3 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as i32) << 24 | (self.inner[self.cursor + 1] as i32) << 16 |
                (self.inner[self.cursor + 2] as i32) << 8 | (self.inner[self.cursor + 3] as i32);
        self.cursor += 4;
        result
    }

    #[inline]
    pub fn read_i64(&mut self) -> i64 {
        if self.cursor + 7 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as i64) << 56 | (self.inner[self.cursor + 1] as i64) << 48 |
                (self.inner[self.cursor + 2] as i64) << 40 | (self.inner[self.cursor + 3] as i64) << 32 |
                (self.inner[self.cursor + 4] as i64) << 24 | (self.inner[self.cursor + 5] as i64) << 16 |
                (self.inner[self.cursor + 6] as i64) << 8 | (self.inner[self.cursor + 7] as i64);
        self.cursor += 8;
        result
    }

    #[inline]
    pub fn read_f32(&mut self) -> f32 {
        unsafe {
            transmute::<i32, f32>(self.read_i32())
        }
    }

    #[inline]
    pub fn read_f64(&mut self) -> f64 {
        unsafe {
            transmute::<i64, f64>(self.read_i64())
        }
    }

    pub fn read_varint(&mut self) -> i32 {
        let mut next: u8 = self.read();
        let mut result: i32 = (next & 0x7F) as i32;
        let mut num_read = 1;

        while next & 0x80 != 0 {
            next = self.read();
            result |= ((next & 0x7F) as i32) << (7 * num_read);
            num_read += 1;
        }

        result
    }

    pub fn read_string(&mut self) -> String {
        let mut bytes: Vec<u8> = vec![0; self.read_varint() as usize];
        self.read_bytes(&mut bytes);
        match str::from_utf8(&bytes) {
            Ok(string) => String::from(string),
            Err(_reason) => String::new()
        }
    }

    #[inline]
    pub fn read_byte_array(&mut self, len: usize) -> Vec<u8> {
        if len == 0 {
            Vec::new()
        } else {
            let mut result = vec![0; len];
            self.read_bytes(&mut result);
            result
        }
    }

    #[inline]
    pub fn write(&mut self, byte: u8) {
        if self.cursor >= self.inner.len() {
            self.inner.push(byte);
        } else {
            self.inner[self.cursor] = byte;
            self.cursor += 1;
        }
    }

    #[inline]
    pub fn write_bytes(&mut self, blob: &[u8]) {
        self.resize(self.cursor + blob.len());
        self.write_bytes_unchecked(blob);
    }

    #[inline]
    pub fn write_bytes_unchecked(&mut self, blob: &[u8]) {
        (self.inner[self.cursor..self.cursor + blob.len()]).copy_from_slice(blob);
        self.cursor += blob.len();
    }

    #[inline]
    pub fn write_bool(&mut self, value: bool) {
        if value {
            self.write(1);
        } else {
            self.write(0);
        }
    }

    #[inline]
    pub fn write_u8(&mut self, value: u8) {
        self.write(value);
    }

    #[inline]
    pub fn write_i8(&mut self, value: i8) {
        self.write(value as u8);
    }

    #[inline]
    pub fn write_u16(&mut self, value: u16) {
        self.ensure_size(self.cursor + 2);
        self.inner[self.cursor] = (value >> 8) as u8;
        self.inner[self.cursor + 1] = value as u8;
    }

    #[inline]
    pub fn write_i16(&mut self, value: i16) {
        self.write_u16(value as u16);
    }

    #[inline]
    pub fn write_i32(&mut self, value: i32) {
        self.ensure_size(self.cursor + 4);
        self.inner[self.cursor] = (value >> 24) as u8;
        self.inner[self.cursor + 1] = (value >> 16) as u8;
        self.inner[self.cursor + 2] = (value >> 8) as u8;
        self.inner[self.cursor + 3] = value as u8;
    }

    #[inline]
    pub fn write_i64(&mut self, value: i64) {
        self.ensure_size(self.cursor + 8);
        self.inner[self.cursor] = (value >> 56) as u8;
        self.inner[self.cursor + 1] = (value >> 48) as u8;
        self.inner[self.cursor + 2] = (value >> 40) as u8;
        self.inner[self.cursor + 3] = (value >> 32) as u8;
        self.inner[self.cursor + 4] = (value >> 24) as u8;
        self.inner[self.cursor + 5] = (value >> 16) as u8;
        self.inner[self.cursor + 6] = (value >> 8) as u8;
        self.inner[self.cursor + 7] = value as u8;
    }

    #[inline]
    pub fn write_f32(&mut self, value: f32) {
        unsafe {
            self.write_i32(transmute::<f32, i32>(value));
        }
    }

    #[inline]
    pub fn write_f64(&mut self, value: f64) {
        unsafe {
            self.write_i64(transmute::<f64, i64>(value));
        }
    }

    #[inline]
    pub fn varint_size(mut value: i32) -> usize {
        match value {
            0..=127 => 1,
            128..=16383 => 2,
            16384..=2097151 => 3,
            2097152..=268435455 => 4,
            _ => 5
        }
    }

    pub fn write_varint(&mut self, mut value: i32) {
        if value == 0 {
            self.write(0);
            return;
        }
    
        let mut next_byte: u8;
    
        while value != 0 {
            next_byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                next_byte |= 0x80;
            }
            self.write(next_byte);
        }
    }

    #[inline]
    pub fn write_string(&mut self, value: &str) {
        let bytes = value.as_bytes();
        self.write_varint(bytes.len() as i32);
        self.ensure_size(self.cursor + value.len());
        self.write_bytes_unchecked(bytes);
    }

    #[inline]
    pub fn write_byte_array(&mut self, value: &[u8]) {
        self.ensure_size(self.cursor + value.len());
        self.write_bytes_unchecked(value);
    }
}