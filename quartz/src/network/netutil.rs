use byteorder::{BigEndian, ByteOrder};
use chat::{Component, TextComponentBuilder};
use quartz_nbt::NbtCompound;
use std::{
    fmt::{self, Debug, Formatter},
    io::Cursor,
    ops::{Index, IndexMut},
    ptr,
    slice::SliceIndex,
    str::{self, FromStr},
};
use util::UnlocalizedName;
use uuid::Uuid;

use crate::{
    network::packets::{EntityMetadata, Particle, PlayerInfoAction},
    world::location::BlockPosition,
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
        unsafe {
            self.inner.set_len(self.inner.capacity());
        }
    }

    /// Ensures that this buffer is at least the given length/size.
    #[inline]
    pub fn ensure_size(&mut self, size: usize) {
        if size > self.inner.capacity() {
            self.inner.reserve(size - self.inner.capacity());
        }

        unsafe {
            self.inner.set_len(size);
        }
    }

    /// Resizes this buffer to the given size.
    #[inline]
    pub fn resize(&mut self, size: usize) {
        self.ensure_size(size);
        if size < self.cursor {
            self.cursor = size;
        }
    }

    /// Returs the position of the cursor in the buffer.
    #[inline]
    pub fn cursor(&self) -> usize {
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

    /// Shifts the remaining bytes after the cursor to the beginning of the buffer.
    #[allow(unsafe_code)]
    pub fn shift_remaining(&mut self) {
        if self.cursor == self.inner.len() {
            self.inner.clear();
            self.cursor = 0;
            return;
        }

        // This was directly copied from vec, so we can assume the standard libraries' devs know
        // what they're doing.
        unsafe {
            let ptr = self.inner.as_mut_ptr();
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
        self.inner.len() - self.cursor()
    }

    /// Clears the contents of this buffer and resets the cursor to the beginning of the buffer.
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear();
        self.cursor = 0;
    }

    /// Returns a mutable reference to the inner vec of this buffer.
    #[inline]
    pub fn inner_mut(&mut self) -> &mut Vec<u8> {
        &mut self.inner
    }

    /// Returns the next byte in the buffer without shifting the cursor. If the cursor is at the end of the
    /// buffer, then `0` is returned.
    #[inline]
    pub fn peek(&self) -> u8 {
        if self.cursor >= self.inner.len() {
            return 0;
        }

        self.inner[self.cursor]
    }

    /// Reads a byte from the buffer, returning `0` if no bytes remain.
    #[inline]
    pub fn read_one(&mut self) -> u8 {
        if self.cursor >= self.inner.len() {
            return 0;
        }

        let byte = self.inner[self.cursor];
        self.cursor += 1;
        byte
    }

    /// Copies bytes from this buffer to the given buffer, returning the number of bytes copied.
    pub fn read_bytes(&mut self, dest: &mut [u8]) -> usize {
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

    pub fn read<T: ReadFromPacket>(&mut self) -> T {
        T::read_from(self)
    }

    pub fn read_varying<T>(&mut self) -> T
    where
        T: VariableRepr,
        <T as VariableRepr>::Wrapper: ReadFromPacket,
    {
        <T as VariableRepr>::Wrapper::read_from(self).into()
    }

    pub fn read_array<T: ReadFromPacket>(&mut self, len: usize) -> Vec<T> {
        let mut dest = Vec::with_capacity(len);
        for _ in 0 .. len {
            dest.push(T::read_from(self));
        }
        dest
    }

    pub fn read_array_varying<T>(&mut self, len: usize) -> Vec<T>
    where
        T: VariableRepr,
        <T as VariableRepr>::Wrapper: ReadFromPacket,
    {
        let mut dest = Vec::with_capacity(len);
        for _ in 0 .. len {
            dest.push(<T as VariableRepr>::Wrapper::read_from(self).into());
        }
        dest
    }

    /// Writes a byte to this buffer, expanding the buffer if needed.
    #[inline]
    pub fn write_one(&mut self, byte: u8) {
        if self.cursor >= self.inner.len() {
            self.inner.push(byte);
        } else {
            self.inner[self.cursor] = byte;
        }
        self.cursor += 1;
    }

    /// Writes the given bytes to this buffer.
    pub fn write_bytes(&mut self, blob: &[u8]) {
        let remaining = self.remaining();
        if remaining < blob.len() {
            let remaining_allocated = self.capacity() - self.cursor;
            if remaining_allocated < blob.len() {
                self.inner.reserve(blob.len() - remaining_allocated);
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

    pub fn write<T: WriteToPacket>(&mut self, value: &T) {
        value.write_to(self)
    }

    pub fn write_varying<T>(&mut self, value: &T)
    where
        T: VariableRepr + Clone,
        <T as VariableRepr>::Wrapper: WriteToPacket,
    {
        <T as VariableRepr>::Wrapper::from(value.clone()).write_to(self)
    }

    pub fn write_array_varying<T>(&mut self, value: &[T])
    where
        T: VariableRepr + Clone,
        <T as VariableRepr>::Wrapper: WriteToPacket,
    {
        for element in value {
            self.write_varying(element);
        }
    }

    /// Returns the number of bytes the given integer would use if encoded as a variable lengthed integer.
    /// Varible length integer can take up anywhere from one to five bytes. If the integer is less than
    /// zero, it will always use five bytes.
    #[inline]
    pub fn varint_size(value: i32) -> usize {
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
        write!(f, "{:X?}", self.inner)
    }
}

pub trait VariableRepr: From<Self::Wrapper> {
    type Wrapper: From<Self>;
}

impl VariableRepr for i32 {
    type Wrapper = Var<i32>;
}

impl From<Var<i32>> for i32 {
    fn from(wrapper: Var<i32>) -> Self {
        wrapper.0
    }
}

impl VariableRepr for i64 {
    type Wrapper = Var<i64>;
}

impl From<Var<i64>> for i64 {
    fn from(wrapper: Var<i64>) -> Self {
        wrapper.0
    }
}

#[repr(transparent)]
pub struct Var<T>(pub T);

impl<T> From<T> for Var<T> {
    fn from(inner: T) -> Self {
        Var(inner)
    }
}

impl ReadFromPacket for Var<i32> {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        let mut next = buffer.read_one();
        let mut result: i32 = (next & 0x7F) as i32;
        let mut num_read = 1;

        while next & 0x80 != 0 {
            next = buffer.read_one();
            result |= ((next & 0x7F) as i32) << (7 * num_read);
            num_read += 1;
        }

        result.into()
    }
}

impl WriteToPacket for Var<i32> {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut value = self.0;

        if value == 0 {
            buffer.write_one(0);
            return;
        }

        let mut next_byte: u8;

        while value != 0 {
            next_byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                next_byte |= 0x80;
            }
            buffer.write_one(next_byte);
        }
    }
}

impl ReadFromPacket for Var<i64> {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        let mut next = buffer.read_one();
        let mut result: i64 = (next & 0x7F) as i64;
        let mut num_read = 1;

        while next & 0x80 != 0 {
            next = buffer.read_one();
            result |= ((next & 0x7F) as i64) << (7 * num_read);
            num_read += 1;
        }

        result.into()
    }
}

impl WriteToPacket for Var<i64> {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut value = self.0;

        if value == 0 {
            buffer.write_one(0);
            return;
        }

        let mut next_byte: u8;

        while value != 0 {
            next_byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                next_byte |= 0x80;
            }
            buffer.write_one(next_byte);
        }
    }
}

pub trait ReadFromPacket {
    fn read_from(buffer: &mut PacketBuffer) -> Self;
}

pub trait WriteToPacket {
    fn write_to(&self, buffer: &mut PacketBuffer);
}

impl ReadFromPacket for u8 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        buffer.read_one()
    }
}

impl ReadFromPacket for bool {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        buffer.read_one() != 0
    }
}

impl ReadFromPacket for i8 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        buffer.read_one() as i8
    }
}

impl ReadFromPacket for u16 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 1 >= buffer.inner.len() {
            return 0;
        }

        let mut buf = [0; 2];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_u16(&buf)
    }
}

impl ReadFromPacket for i16 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 1 >= buffer.inner.len() {
            return 0;
        }

        let mut buf = [0; 2];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_i16(&buf)
    }
}

impl ReadFromPacket for i32 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 3 >= buffer.inner.len() {
            return 0;
        }

        let mut buf = [0; 4];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_i32(&buf)
    }
}

impl ReadFromPacket for i64 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 7 >= buffer.inner.len() {
            return 0;
        }

        let mut buf = [0; 8];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_i64(&buf)
    }
}

impl ReadFromPacket for u128 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 15 >= buffer.inner.len() {
            return 0;
        }

        let mut buf = [0; 16];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_u128(&buf)
    }
}

impl ReadFromPacket for f32 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 3 >= buffer.inner.len() {
            return 0.0;
        }

        let mut buf = [0; 4];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_f32(&buf)
    }
}

impl ReadFromPacket for f64 {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        if buffer.cursor + 7 >= buffer.inner.len() {
            return 0.0;
        }

        let mut buf = [0; 8];
        // Safety: length check performed above
        unsafe {
            buffer.read_bytes_unchecked(&mut buf);
        }

        BigEndian::read_f64(&buf)
    }
}

impl ReadFromPacket for String {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        let mut bytes: Vec<u8> =
            vec![0; (buffer.read_varying::<i32>() as usize).min(buffer.remaining())];

        // Safety: amount of bytes read is limited above
        unsafe {
            buffer.read_bytes_unchecked(&mut bytes);
        }

        match str::from_utf8(&bytes) {
            Ok(string) => string.to_owned(),
            Err(_reason) => String::new(),
        }
    }
}

impl ReadFromPacket for BlockPosition {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        let long = buffer.read::<i64>() as u64;

        let x = (long >> 38) as i32;
        let y = (long & 0xFFF) as i16;
        let z = (long << 26 >> 38) as i32;

        BlockPosition { x, y, z }
    }
}

impl ReadFromPacket for Uuid {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        Uuid::from_bytes(buffer.read::<u128>().to_be_bytes())
    }
}

impl ReadFromPacket for UnlocalizedName {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        UnlocalizedName::from_str(&buffer.read::<String>())
            .unwrap_or(UnlocalizedName::minecraft("air"))
    }
}

impl ReadFromPacket for NbtCompound {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        let mut cursor = Cursor::new(&buffer.inner);
        cursor.set_position(buffer.cursor as u64);
        let ret = match quartz_nbt::read::read_nbt_uncompressed(&mut cursor) {
            Ok((nbt, _)) => nbt,
            Err(_) => NbtCompound::new(),
        };
        buffer.cursor = cursor.position() as usize;
        ret
    }
}

impl ReadFromPacket for Component {
    fn read_from(buffer: &mut PacketBuffer) -> Self {
        serde_json::from_str(&buffer.read::<String>())
            .unwrap_or(TextComponentBuilder::new(String::new()).build())
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
}

impl WriteToPacket for i64 {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        let mut buf = [0; 8];
        BigEndian::write_i64(&mut buf, *self);
        buffer.write_bytes(&buf);
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

impl<T: WriteToPacket> WriteToPacket for Vec<T> {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        for element in self {
            element.write_to(buffer);
        }
    }
}

impl WriteToPacket for BlockPosition {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(
            &((((self.x as u64 & 0x3FFFFFF) << 38)
                | ((self.z as u64 & 0x3FFFFFF) << 12)
                | (self.y as u64 & 0xFFF)) as i64),
        );
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
        let mut cursor = Cursor::new(&mut buffer.inner);
        cursor.set_position(buffer.cursor as u64);
        let _ = quartz_nbt::write::write_nbt_uncompressed(&mut cursor, "root", self);
        buffer.cursor = cursor.position() as usize;
    }
}

impl WriteToPacket for EntityMetadata {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        match self {
            EntityMetadata::Byte(v) => buffer.write(v),
            EntityMetadata::VarInt(v) => buffer.write_varying(v),
            EntityMetadata::Float(v) => buffer.write(v),
            EntityMetadata::String(v) => buffer.write(v),
            EntityMetadata::Chat(v) => buffer.write(v),
            EntityMetadata::OptChat(b, c) => {
                buffer.write(b);
                match c {
                    Some(c) => buffer.write(c),
                    None => {}
                }
            }
            EntityMetadata::Slot(v) => buffer.write(v),
            EntityMetadata::Boolean(v) => buffer.write(v),
            EntityMetadata::Rotation(x, y, z) => {
                buffer.write(x);
                buffer.write(y);
                buffer.write(z);
            }
            EntityMetadata::Position(v) => buffer.write(v),
            EntityMetadata::OptPosition(b, p) => {
                buffer.write(b);
                match p {
                    Some(p) => buffer.write(p),
                    None => {}
                }
            }
            EntityMetadata::Direction(v) => buffer.write_varying(v),
            EntityMetadata::OptUUID(b, u) => {
                buffer.write(b);
                match u {
                    Some(u) => buffer.write(u),
                    None => {}
                }
            }
            EntityMetadata::OptBlockId(v) => buffer.write_varying(v),
            EntityMetadata::NBT(v) => buffer.write(v),
            EntityMetadata::Particle(v) => buffer.write(v),
            EntityMetadata::VillagerData(a, b, c) => {
                buffer.write(a);
                buffer.write(b);
                buffer.write(c);
            }
            EntityMetadata::OptVarInt(v) => buffer.write_varying(v),
            EntityMetadata::Pose(v) => buffer.write_varying(v),
        }
    }
}

impl WriteToPacket for Particle {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        match self {
            Particle::Block(v) => buffer.write_varying(v),
            Particle::Dust(x, y, z, s) => {
                buffer.write(x);
                buffer.write(y);
                buffer.write(z);
                buffer.write(s);
            }
            Particle::FallingDust(v) => buffer.write_varying(v),
            Particle::Item(v) => buffer.write(v),
            _ => {}
        }
    }
}

impl WriteToPacket for PlayerInfoAction {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        match self {
            PlayerInfoAction::AddPlayer {
                name,
                number_of_properties,
                properties,
                gamemode,
                ping,
                has_display_name,
                display_name,
            } => {
                buffer.write(name);
                buffer.write_varying(number_of_properties);
                buffer.write(properties);
                buffer.write_varying(gamemode);
                buffer.write_varying(ping);
                buffer.write(has_display_name);
                match display_name {
                    Some(v) => buffer.write(v),
                    None => {}
                }
            }
            PlayerInfoAction::UpdateGamemode { gamemode } => {
                buffer.write_varying(gamemode);
            }
            PlayerInfoAction::UpdateLatency { ping } => {
                buffer.write_varying(ping);
            }
            PlayerInfoAction::UpdateDisplayName {
                has_display_name,
                display_name,
            } => {
                buffer.write(has_display_name);
                match display_name {
                    Some(v) => buffer.write(v),
                    None => {}
                }
            }
            PlayerInfoAction::RemovePlayer => {}
        }
    }
}

impl WriteToPacket for Component {
    fn write_to(&self, buffer: &mut PacketBuffer) {
        buffer.write(&serde_json::to_string(self).unwrap_or(String::new()));
    }
}
