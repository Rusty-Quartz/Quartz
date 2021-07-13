use byteorder::{BigEndian, ByteOrder};
use openssl::error::ErrorStack;
use quartz_chat::Component;
use quartz_nbt::NbtCompound;
use quartz_util::UnlocalizedName;
use std::{
    error::Error,
    fmt::{self, Debug, Display, Formatter},
    io,
    io::Cursor,
    ops::{Index, IndexMut},
    ptr,
    slice::SliceIndex,
    str::{self, FromStr, Utf8Error},
};
use uuid::Uuid;

use crate::world::{
    chunk::{ClientSection},
    location::BlockPosition,
};

/// A wrapper around a vec used for reading/writing packet data efficiently.
pub struct PacketBuffer {
    inner: Vec<u8>,
    cursor: usize,
}

impl PacketBuffer {
    /// Creates a new packet buffer with the given initial capacity.
    pub fn new(initial_capacity: usize) -> Self {
        PacketBuffer {
            inner: Vec::with_capacity(initial_capacity),
            cursor: 0,
        }
    }

    /// Returns the capacity of this buffer.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// Return the length of this buffer.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Increases the length of this buffer to its capacity.
    #[inline]
    pub fn inflate(&mut self) {
        // Safety: u8 is always valid, and we only set the length up to the amount we have allocated
        unsafe {
            self.inner.set_len(self.inner.capacity());
        }
    }

    /// Resizes this buffer to the given size.
    #[inline]
    pub fn resize(&mut self, size: usize) {
        if size > self.inner.capacity() {
            // inner cap >= inner len + size - inner len = size
            self.inner.reserve(size - self.inner.len());
        }

        // Safety: see computation above
        unsafe {
            self.inner.set_len(size);
        }

        if size < self.cursor {
            self.cursor = size;
        }
    }

    pub fn ensure_remaining(&mut self, n: usize) {
        if n <= self.remaining() {
            return;
        }

        // inner cap >= inner len + n - (inner len - cursor) = n + cursor
        self.inner.reserve(n - self.remaining());

        // Safety: see computation above
        unsafe {
            self.inner.set_len(self.cursor + n);
        }
    }

    /// Returs the position of the cursor in the buffer.
    #[inline]
    pub fn cursor(&self) -> usize {
        debug_assert!(self.cursor <= self.len());
        self.cursor
    }

    /// Sets this buffer's cursor to the beginning of the buffer.
    #[inline]
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }

    /// Sets the position of this buffer's cursor to the given position. If the given position is greater
    /// than the length of this buffer, then the curser is set to the buffer's length.
    #[inline]
    pub fn set_cursor(&mut self, cursor: usize) {
        if cursor > self.inner.len() {
            self.cursor = self.inner.len();
        } else {
            self.cursor = cursor;
        }
    }

    #[inline]
    pub unsafe fn set_len(&mut self, new_len: usize) {
        self.inner.set_len(new_len);
    }

    /// Shifts the remaining bytes after the cursor to the beginning of the buffer.
    pub fn shift_remaining(&mut self) {
        if self.cursor >= self.inner.len() {
            self.inner.clear();
            self.cursor = 0;
            return;
        }

        // This was directly copied from vec, so we can assume the standard libraries' devs know
        // what they're doing.
        unsafe {
            let ptr = self.inner.as_mut_ptr();
            // Subtraction checked above
            let new_len = self.inner.len() - self.cursor;
            // Copy remaining bytes
            ptr::copy(ptr.add(self.cursor), ptr, new_len);
            self.inner.set_len(new_len);
            self.cursor = 0;
        }
    }

    /// Returns the number of bytes remaining in this buffer.
    #[inline]
    pub fn remaining(&self) -> usize {
        self.inner.len().checked_sub(self.cursor).unwrap_or(0)
    }

    /// Clears the contents of this buffer and resets the cursor to the beginning of the buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
        self.cursor = 0;
    }

    /// Returns a mutable reference to the inner vec of this buffer. Even though this operation is not
    /// inherently unsafe, incorrectly modifying the returned reference can lead to undefined behavior
    /// down the line if the cursor position is not handled correctly.
    #[inline]
    pub unsafe fn inner_mut(&mut self) -> &mut Vec<u8> {
        &mut self.inner
    }

    /// Returns the next byte in the buffer without shifting the cursor. If the cursor is at the end of the
    /// buffer, then `0` is returned.
    #[inline]
    pub fn peek_one(&self) -> Result<u8, PacketSerdeError> {
        self.inner
            .get(self.cursor)
            .copied()
            .ok_or(PacketSerdeError::EndOfBuffer)
    }

    pub fn peek<T: ReadFromPacket>(&mut self) -> Result<T, PacketSerdeError> {
        let cursor_start = self.cursor;
        let result = self.read::<T>();
        self.cursor = cursor_start;
        result
    }

    pub fn peek_varying<T: ReadFromPacket>(&mut self) -> Result<T, PacketSerdeError> {
        let cursor_start = self.cursor;
        let result = self.read_varying::<T>();
        self.cursor = cursor_start;
        result
    }

    /// Reads a byte from the buffer, returning `0` if no bytes remain.
    #[inline]
    pub fn read_one(&mut self) -> Result<u8, PacketSerdeError> {
        match self.inner.get(self.cursor).copied() {
            Some(by) => {
                self.cursor += 1;
                Ok(by)
            }
            None => Err(PacketSerdeError::EndOfBuffer),
        }
    }

    /// Copies bytes from this buffer to the given buffer, returning the number of bytes copied.
    pub fn read_bytes<T: AsMut<[u8]>>(&mut self, mut dest: T) -> usize {
        let dest = dest.as_mut();
        let len = self.remaining().min(dest.len());
        unsafe {
            self.read_bytes_unchecked(&mut dest[.. len]);
        }
        len
    }

    /// Fills the given vec with bytes from this buffer. Note that this function has no bounds checks.
    #[inline]
    unsafe fn read_bytes_unchecked(&mut self, dest: &mut [u8]) {
        let len = dest.len();
        let src = self.inner.as_ptr().add(self.cursor);
        let dest = dest.as_mut_ptr();
        ptr::copy_nonoverlapping(src, dest, len);
        self.cursor += len;
    }

    #[inline]
    pub fn read<T: ReadFromPacket>(&mut self) -> Result<T, PacketSerdeError> {
        T::read_from(self)
    }

    #[inline]
    pub fn read_varying<T: ReadFromPacket>(&mut self) -> Result<T, PacketSerdeError> {
        T::varying_read_from(self)
    }

    #[inline]
    pub fn read_array<T: ReadFromPacket>(
        &mut self,
        len: usize,
    ) -> Result<Box<[T]>, PacketSerdeError> {
        let mut dest = Vec::with_capacity(len);
        for _ in 0 .. len {
            dest.push(T::read_from(self)?);
        }
        Ok(dest.into_boxed_slice())
    }

    #[inline]
    pub fn read_array_varying<T: ReadFromPacket>(
        &mut self,
        len: usize,
    ) -> Result<Box<[T]>, PacketSerdeError> {
        let mut dest = Vec::with_capacity(len);
        for _ in 0 .. len {
            dest.push(T::varying_read_from(self)?);
        }
        Ok(dest.into_boxed_slice())
    }

    #[inline]
    pub fn read_mc_str(&mut self) -> Result<&str, PacketSerdeError> {
        let byte_len = self.read_varying::<i32>()? as usize;
        if self.cursor + byte_len > self.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let ret =
            str::from_utf8(&self.inner[self.cursor .. self.cursor + byte_len]).map_err(Into::into);
        self.cursor += byte_len;
        ret
    }

    /// Writes a byte to this buffer, expanding the buffer if needed.
    #[inline]
    pub fn write_one(&mut self, byte: u8) {
        match self.inner.get_mut(self.cursor) {
            Some(by) => *by = byte,
            None => {
                debug_assert!(self.cursor == self.inner.len());
                self.inner.push(byte);
            }
        }
        self.cursor += 1;
    }

    /// Writes the given bytes to this buffer.
    pub fn write_bytes<T: AsRef<[u8]>>(&mut self, blob: T) {
        debug_assert!(self.cursor <= self.len());

        let blob = blob.as_ref();
        let remaining = self.remaining();
        if remaining < blob.len() {
            let remaining_allocated = self.capacity() - self.cursor;
            if remaining_allocated < blob.len() {
                // inner cap >= inner len + blob len - (inner len - cursor) = blob len + cursor
                self.inner.reserve(blob.len() - remaining);
            }

            // Safety: above we allocate enough memory to fit `blob`, and after the write is
            // performed the end of the valid data in this buffer will be at the index below
            unsafe {
                self.inner.set_len(self.cursor + blob.len());
            }
        }

        // Safety: sufficient allocations were performed above
        unsafe {
            self.write_bytes_unchecked(blob);
        }
    }

    /// Writes the given bytes to this buffer without performing size checks.
    #[inline]
    unsafe fn write_bytes_unchecked(&mut self, blob: &[u8]) {
        let len = blob.len();
        let src = blob.as_ptr();
        let dest = self.inner.as_mut_ptr().add(self.cursor);
        ptr::copy_nonoverlapping(src, dest, len);
        self.cursor += len;
    }

    #[inline]
    pub fn write<T: WriteToPacket>(&mut self, value: &T) {
        value.write_to(self);
    }

    #[inline]
    pub fn write_varying<T: WriteToPacket>(&mut self, value: &T) {
        value.varying_write_to(self);
    }

    #[inline]
    pub fn write_array<T: WriteToPacket>(&mut self, value: &[T]) {
        for element in value {
            element.write_to(self);
        }
    }

    #[inline]
    pub fn write_array_varying<T: WriteToPacket>(&mut self, value: &[T]) {
        for element in value {
            element.varying_write_to(self);
        }
    }

    /// Returns the number of bytes the given integer would use if encoded as a variable lengthed integer.
    /// Varible length integer can take up anywhere from one to five bytes. If the integer is less than
    /// zero, it will always use five bytes.
    #[inline]
    pub const fn varint_size(value: i32) -> usize {
        match value {
            0 ..= 127 => 1,
            128 ..= 16383 => 2,
            16384 ..= 2097151 => 3,
            2097152 ..= 268435455 => 4,
            _ => 5,
        }
    }
}

impl<Idx> Index<Idx> for PacketBuffer
where Idx: SliceIndex<[u8]>
{
    type Output = Idx::Output;

    fn index(&self, index: Idx) -> &Self::Output {
        &self.inner[index]
    }
}

impl<Idx> IndexMut<Idx> for PacketBuffer
where Idx: SliceIndex<[u8]>
{
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output {
        &mut self.inner[index]
    }
}

impl Debug for PacketBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:02X?}", self.inner)
    }
}

pub trait ReadFromPacket: Sized {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError>;

    fn varying_read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        Self::read_from(buffer)
    }
}

impl<T: ReadFromPacket> ReadFromPacket for Box<T> {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        T::read_from(buffer).map(Box::new)
    }

    fn varying_read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        T::varying_read_from(buffer).map(Box::new)
    }
}

pub trait WriteToPacket {
    fn write_to(&self, buffer: &mut PacketBuffer);

    fn varying_write_to(&self, buffer: &mut PacketBuffer) {
        self.write_to(buffer);
    }
}

impl<T: WriteToPacket> WriteToPacket for Box<T> {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        (&**self).write_to(buffer)
    }

    fn varying_write_to(&self, buffer: &mut PacketBuffer) {
        (&**self).varying_write_to(buffer)
    }
}

impl ReadFromPacket for u8 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        buffer.read_one()
    }
}

impl ReadFromPacket for bool {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        Ok(buffer.read_one()? != 0)
    }
}

impl ReadFromPacket for i8 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        Ok(buffer.read_one()? as i8)
    }
}

impl ReadFromPacket for u16 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 1 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 2];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_u16(&buf))
    }
}

impl ReadFromPacket for i16 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 1 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 2];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_i16(&buf))
    }
}

impl ReadFromPacket for i32 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 3 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 4];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_i32(&buf))
    }

    fn varying_read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let mut buf = [0u8; 5];

        let remaining = buffer.remaining();
        let len = remaining.min(buf.len());
        unsafe {
            let src = buffer.inner.as_ptr().add(buffer.cursor());
            ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), len);
        }

        let mut result = 0i32;
        let mut by;
        let mut i = 0;

        while i < buf.len() {
            by = buf[i];
            result |= ((by & 0x7F) as i32) << (7 * i);
            i += 1;

            if i > remaining {
                return Err(PacketSerdeError::EndOfBuffer);
            }

            if (by & 0x80) == 0 {
                buffer.cursor += i;
                return Ok(result);
            }
        }

        Err(PacketSerdeError::VarIntOverflow)
    }
}

impl ReadFromPacket for i64 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 7 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 8];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_i64(&buf))
    }

    fn varying_read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let mut buf = [0u8; 10];

        let remaining = buffer.remaining();
        let len = remaining.min(buf.len());
        unsafe {
            let src = buffer.inner.as_ptr().add(buffer.cursor());
            ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), len);
        }

        let mut result = 0i64;
        let mut by;
        let mut i = 0;

        while i < buf.len() {
            by = buf[i];
            result |= ((by & 0x7F) as i64) << (7 * i);
            i += 1;

            if i > remaining {
                return Err(PacketSerdeError::EndOfBuffer);
            }

            if (by & 0x80) == 0 {
                buffer.cursor += i;
                return Ok(result);
            }
        }

        Err(PacketSerdeError::VarIntOverflow)
    }
}

impl ReadFromPacket for u128 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 15 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 16];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_u128(&buf))
    }
}

impl ReadFromPacket for f32 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 3 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 4];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_f32(&buf))
    }
}

impl ReadFromPacket for f64 {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        if buffer.cursor() + 7 >= buffer.inner.len() {
            return Err(PacketSerdeError::EndOfBuffer);
        }

        let mut buf = [0; 8];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        Ok(BigEndian::read_f64(&buf))
    }
}

impl ReadFromPacket for String {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        buffer.read_mc_str().map(ToOwned::to_owned)
    }
}

impl ReadFromPacket for BlockPosition {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        Ok(BlockPosition::from_u64(buffer.read::<i64>()? as u64))
    }
}

impl ReadFromPacket for Uuid {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        Ok(Uuid::from_bytes(buffer.read::<u128>()?.to_be_bytes()))
    }
}

impl ReadFromPacket for UnlocalizedName {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        match UnlocalizedName::from_str(buffer.read_mc_str()?) {
            Ok(string) => Ok(string.to_owned()),
            Err(error) => Err(PacketSerdeError::InvalidUnlocalizedName(error)),
        }
    }
}

impl ReadFromPacket for NbtCompound {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let mut cursor = Cursor::new(&buffer.inner);
        cursor.set_position(buffer.cursor() as u64);
        let ret = match quartz_nbt::read::read_nbt_uncompressed(&mut cursor) {
            Ok((nbt, _)) => Ok(nbt),
            Err(error) => Err(PacketSerdeError::Nbt(error)),
        };
        let position = cursor.position() as usize;
        buffer.set_cursor(position);
        ret
    }
}

impl ReadFromPacket for Component {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        serde_json::from_str(buffer.read_mc_str()?).map_err(Into::into)
    }
}

impl WriteToPacket for u8 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write_one(*self)
    }
}

impl WriteToPacket for bool {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        if *self {
            buffer.write_one(1);
        } else {
            buffer.write_one(0);
        }
    }
}

impl WriteToPacket for i8 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write_one(*self as u8)
    }
}

impl WriteToPacket for u16 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 2];
        BigEndian::write_u16(&mut buf, *self);
        buffer.write_bytes(&buf);
    }
}

impl WriteToPacket for i16 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 2];
        BigEndian::write_i16(&mut buf, *self);
        buffer.write_bytes(&buf);
    }
}

impl WriteToPacket for i32 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 4];
        BigEndian::write_i32(&mut buf, *self);
        buffer.write_bytes(&buf);
    }

    fn varying_write_to(&self, buffer: &mut PacketBuffer) {
        let mut value = *self as u32;
        let mut next_byte: u8;
        let mut buf = [0u8; 5];
        let mut i = 0;

        while i < 5 {
            next_byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                next_byte |= 0x80;
            }
            buf[i] = next_byte;
            i += 1;

            if value == 0 {
                break;
            }
        }

        buffer.write_bytes(&buf[.. i]);
    }
}

impl WriteToPacket for i64 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 8];
        BigEndian::write_i64(&mut buf, *self);
        buffer.write_bytes(&buf);
    }

    fn varying_write_to(&self, buffer: &mut PacketBuffer) {
        let mut value = *self as u64;
        let mut next_byte: u8;
        let mut buf = [0u8; 10];
        let mut i = 0;

        while i < 10 {
            next_byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                next_byte |= 0x80;
            }
            buf[i] = next_byte;
            i += 1;

            if value == 0 {
                break;
            }
        }

        buffer.write_bytes(&buf[.. i]);
    }
}

impl WriteToPacket for u128 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 16];
        BigEndian::write_u128(&mut buf, *self);
        buffer.write_bytes(&buf);
    }
}

impl WriteToPacket for f32 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 4];
        BigEndian::write_f32(&mut buf, *self);
        buffer.write_bytes(&buf);
    }
}

impl WriteToPacket for f64 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 8];
        BigEndian::write_f64(&mut buf, *self);
        buffer.write_bytes(&buf);
    }
}

impl WriteToPacket for String {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let bytes = self.as_bytes();
        buffer.write_varying(&(bytes.len() as i32));
        buffer.write_bytes(bytes);
    }
}

impl WriteToPacket for &str {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let bytes = self.as_bytes();
        buffer.write_varying(&(bytes.len() as i32));
        buffer.write_bytes(bytes);
    }
}

impl WriteToPacket for BlockPosition {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&(self.as_u64() as i64));
    }
}

impl WriteToPacket for Uuid {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&self.as_u128());
    }
}

impl WriteToPacket for UnlocalizedName {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&self.to_string());
    }
}

impl WriteToPacket for NbtCompound {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let position = buffer.cursor();
        let mut cursor = Cursor::new(&mut buffer.inner);
        cursor.set_position(position as u64);
        let _ = quartz_nbt::write::write_nbt_uncompressed(&mut cursor, "", self);
        let position = cursor.position() as usize;
        buffer.set_cursor(position);
    }
}

impl WriteToPacket for Component {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&serde_json::to_string(self).unwrap_or(String::new()));
    }
}

impl WriteToPacket for ClientSection {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&self.block_count);
        buffer.write(&self.bits_per_block);
        if let Some(palette) = &self.palette {
            buffer.write_varying(&(palette.len() as i32));
            buffer.write_array_varying(palette);
        }
        buffer.write_varying(&(self.data.len() as i64));
        buffer.write_array(&self.data)
    }
}

impl ReadFromPacket for ClientSection {
    fn read_from(buffer: &mut PacketBuffer) -> Result<Self, PacketSerdeError> {
        let block_count = buffer.read()?;
        let bits_per_block = match buffer.read()? {
            0 ..= 4 => 4,
            b @ _ => b,
        };
        let palette = if bits_per_block < 9 {
            let palette_len: i32 = buffer.read_varying()?;
            Some(buffer.read_array_varying(palette_len as usize)?)
        } else {
            None
        };
        let data_len: i32 = buffer.read_varying()?;
        let mut data = Vec::new();
        for _ in 0 .. data_len {
            data.push(buffer.read()?);
        }

        Ok(ClientSection {
            block_count,
            palette,
            bits_per_block,
            data: data.into_boxed_slice(),
        })
    }
}

// TODO: add From<NbtIoError> when quartz_nbt is updated
#[derive(Debug)]
pub enum PacketSerdeError {
    EndOfBuffer,
    VarIntOverflow,
    InvalidId(i32),
    Utf8Error(Utf8Error),
    InvalidUnlocalizedName(<UnlocalizedName as FromStr>::Err),
    SerdeJson(serde_json::Error),
    Nbt(io::Error),
    Network(io::Error),
    OpenSSL(ErrorStack),
    InvalidEnum(&'static str, i32),
    Internal(&'static str),
    InvalidRecipe(Box<str>),
}

impl Display for PacketSerdeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            PacketSerdeError::EndOfBuffer => write!(f, "Unexpectedly reached end of packet buffer"),
            PacketSerdeError::VarIntOverflow => write!(
                f,
                "Variable-length integer or long overflowed while reading"
            ),
            PacketSerdeError::InvalidId(id) => write!(f, "Invalid packet ID encountered: {}", id),
            PacketSerdeError::Utf8Error(e) => Display::fmt(e, f),
            PacketSerdeError::InvalidUnlocalizedName(uln) => write!(
                f,
                "Invalid unlocalized name encountered while reading: \"{}\"",
                uln
            ),
            PacketSerdeError::SerdeJson(e) => Display::fmt(e, f),
            PacketSerdeError::Nbt(e) => Display::fmt(e, f),
            PacketSerdeError::Network(e) => Display::fmt(e, f),
            PacketSerdeError::OpenSSL(e) => Display::fmt(e, f),
            PacketSerdeError::InvalidEnum(enum_type, id) =>
                write!(f, "Received invalid enum ID for type {}: {}", enum_type, id),
            PacketSerdeError::Internal(msg) => Display::fmt(msg, f),
            PacketSerdeError::InvalidRecipe(msg) => Display::fmt(msg, f),
        }
    }
}

impl Error for PacketSerdeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            PacketSerdeError::Utf8Error(e) => Some(e),
            PacketSerdeError::SerdeJson(e) => Some(e),
            PacketSerdeError::Nbt(e) => Some(e),
            PacketSerdeError::Network(e) => Some(e),
            PacketSerdeError::OpenSSL(e) => Some(e),
            _ => None,
        }
    }
}

impl From<Utf8Error> for PacketSerdeError {
    fn from(error: Utf8Error) -> Self {
        PacketSerdeError::Utf8Error(error)
    }
}

impl From<serde_json::Error> for PacketSerdeError {
    fn from(error: serde_json::Error) -> Self {
        PacketSerdeError::SerdeJson(error)
    }
}

impl From<ErrorStack> for PacketSerdeError {
    fn from(error: ErrorStack) -> Self {
        PacketSerdeError::OpenSSL(error)
    }
}
