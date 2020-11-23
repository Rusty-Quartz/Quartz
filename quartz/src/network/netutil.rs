use chat::Component;
use nbt::NbtCompound;
use std::{
    fmt::{self, Display, Formatter},
    ops::{Index, IndexMut},
    ptr,
    slice::SliceIndex,
    str::{self, FromStr},
};
use util::UnlocalizedName;
use uuid::Uuid;

use crate::world::location::BlockPosition;

use super::{EntityMetadata, Particle, PlayerInfoAction};

/// A wrapper around a vec used for reading/writing packet data efficiently.
pub struct PacketBuffer {
    inner: Vec<u8>,
    cursor: usize,
}

impl From<&[u8]> for PacketBuffer {
    fn from(bytes: &[u8]) -> Self {
        PacketBuffer {
            inner: Vec::from(bytes),
            cursor: 0,
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

impl Display for PacketBuffer {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:X?}", self.inner)
    }
}

impl PacketBuffer {
    /// Creates a new packet buffer with the given initial capacity.
    pub fn new(initial_size: usize) -> Self {
        PacketBuffer {
            inner: Vec::with_capacity(initial_size),
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
        self.inner.resize(self.inner.capacity(), 0);
    }

    /// Ensures that this buffer is at least the given length/size.
    #[inline]
    pub fn ensure_size(&mut self, size: usize) {
        self.inner.resize(size, 0);
    }

    /// Resizes this buffer to the given size.
    #[inline]
    pub fn resize(&mut self, size: usize) {
        self.inner.resize(size, 0);
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
    pub fn read(&mut self) -> u8 {
        if self.cursor >= self.inner.len() {
            return 0;
        }

        let byte = self.inner[self.cursor];
        self.cursor += 1;
        byte
    }

    /// Fills the given vec with bytes from this buffer. Note that this function has no bounds checks.
    #[inline]
    fn read_bytes(&mut self, dest: &mut Vec<u8>) {
        let len = dest.len();
        dest.copy_from_slice(&self.inner[self.cursor .. self.cursor + len]);
        self.cursor += len;
    }

    /// Reads a boolean from the buffer.
    #[inline]
    pub fn read_bool(&mut self) -> bool {
        self.read() != 0
    }

    /// An alias for the `read` function.
    #[inline]
    pub fn read_u8(&mut self) -> u8 {
        self.read()
    }

    /// Reads a signed byte from this buffer, returning `0` if no bytes remain.
    #[inline]
    pub fn read_i8(&mut self) -> i8 {
        self.read() as i8
    }

    /// Reads an unsigned two-byte integer from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_u16(&mut self) -> u16 {
        if self.cursor + 1 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as u16) << 8 | (self.inner[self.cursor + 1] as u16);
        self.cursor += 2;
        result
    }

    /// Reads a signed two-byte integer from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_i16(&mut self) -> i16 {
        self.read_u16() as i16
    }

    /// Reads a signed four-byte integer from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_i32(&mut self) -> i32 {
        if self.cursor + 3 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as i32) << 24
            | (self.inner[self.cursor + 1] as i32) << 16
            | (self.inner[self.cursor + 2] as i32) << 8
            | (self.inner[self.cursor + 3] as i32);
        self.cursor += 4;
        result
    }

    /// Reads a signed eight-byte integer from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_i64(&mut self) -> i64 {
        if self.cursor + 7 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as i64) << 56
            | (self.inner[self.cursor + 1] as i64) << 48
            | (self.inner[self.cursor + 2] as i64) << 40
            | (self.inner[self.cursor + 3] as i64) << 32
            | (self.inner[self.cursor + 4] as i64) << 24
            | (self.inner[self.cursor + 5] as i64) << 16
            | (self.inner[self.cursor + 6] as i64) << 8
            | (self.inner[self.cursor + 7] as i64);
        self.cursor += 8;
        result
    }

    /// Reads an unsigned 16-byte integer from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_u128(&mut self) -> u128 {
        if self.cursor + 15 >= self.inner.len() {
            return 0;
        }

        let result = (self.inner[self.cursor] as u128) << 120
            | (self.inner[self.cursor + 1] as u128) << 112
            | (self.inner[self.cursor + 2] as u128) << 104
            | (self.inner[self.cursor + 3] as u128) << 96
            | (self.inner[self.cursor + 4] as u128) << 88
            | (self.inner[self.cursor + 5] as u128) << 80
            | (self.inner[self.cursor + 6] as u128) << 72
            | (self.inner[self.cursor + 7] as u128) << 64
            | (self.inner[self.cursor + 8] as u128) << 56
            | (self.inner[self.cursor + 9] as u128) << 48
            | (self.inner[self.cursor + 10] as u128) << 40
            | (self.inner[self.cursor + 11] as u128) << 32
            | (self.inner[self.cursor + 12] as u128) << 24
            | (self.inner[self.cursor + 13] as u128) << 16
            | (self.inner[self.cursor + 14] as u128) << 8
            | (self.inner[self.cursor + 15] as u128);
        self.cursor += 16;
        result
    }

    /// Reads a 32-bit float from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_f32(&mut self) -> f32 {
        f32::from_bits(self.read_i32() as u32)
    }

    /// Reads a 64-bit float from this buffer, returning `0` if not enough bytes remain.
    #[inline]
    pub fn read_f64(&mut self) -> f64 {
        f64::from_bits(self.read_i64() as u64)
    }

    /// Reads a variable length, signed integer from this buffer. Bits will continue to be pushed onto
    /// the resulting integer as long as the most signifint bit in each successive byte is set to one.
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

    /// Reads a variable length, signed long from this buffer. Bits will continue to be pushed onto
    /// the resulting long as long as the most signifint bit in each successive byte is set to one.
    pub fn read_varlong(&mut self) -> i64 {
        let mut next = self.read();
        let mut result: i64 = (next & 0x7F) as i64;
        let mut num_read = 1;

        while next & 0x80 != 0 {
            next = self.read();
            result |= ((next & 0x7F) as i64) << (7 * num_read);
            num_read += 1;
        }

        result
    }

    /// Reads a length-prefixed string from the buffer. The length is encoded by a variable length integer.
    pub fn read_string(&mut self) -> String {
        let mut bytes: Vec<u8> = vec![0; self.read_varint() as usize];
        self.read_bytes(&mut bytes);
        match str::from_utf8(&bytes) {
            Ok(string) => string.to_owned(),
            Err(_reason) => String::new(),
        }
    }

    /// Reads a byte array up to the given length from this buffer. If less than the given length of bytes
    /// remain, then all of the remaining bytes are returned.
    #[inline]
    pub fn read_byte_array(&mut self, len: usize) -> Vec<u8> {
        if len == 0 {
            Vec::new()
        } else {
            let mut result = vec![0; len.min(self.remaining())];
            self.read_bytes(&mut result);
            result
        }
    }

    /// Reads an array of type `T` using `func` until `len` is met. If `len` cannot be met the last element is invalid.
    pub fn read_array<T>(&mut self, len: usize, func: fn(&mut Self) -> T) -> Vec<T> {
        if len == 0 {
            Vec::new()
        } else {
            let mut count = 0;
            let mut result = Vec::new();

            while count < len && self.cursor < self.inner.len() {
                result.push(func(self));
                count += 1;
            }

            result
        }
    }

    /// Reads a [`BlockPosition`](crate::world::location::BlockPosition) from an eight byte integer in this buffer.
    pub fn read_position(&mut self) -> BlockPosition {
        let long = self.read_i64();

        let x = (long >> 38) as i32;
        let y = (long & 0xFFF) as i16;
        let z = (long << 26 >> 38) as i32;

        crate::world::location::BlockPosition { x, y, z }
    }

    /// Reads a [`UUID`](util::Uuid) from a 16-byte integer in this buffer.
    pub fn read_uuid(&mut self) -> Uuid {
        Uuid::from_bytes(self.read_u128().to_be_bytes())
    }

    /// Reads an [`UnlocalizedName`](util::UnlocalizedName) from a string stored in this buffer
    pub fn read_unlocalized_name(&mut self) -> UnlocalizedName {
        let str = self.read_string();
        UnlocalizedName::from_str(&str).unwrap()
    }

    /// Reads a [`NbtCompound`](nbt::NbtCompound) from bytes stored in this buffer
    /// Does not expect compression
    pub fn read_nbt_tag(&mut self) -> NbtCompound {
        nbt::read::read_nbt_uncompressed(&mut self.inner.as_slice())
            .expect("Error reading nbt compound")
            .0
    }

    /// Never used and is unimplemented
    pub fn read_entity_metadata(&mut self, _var_type: i32) -> EntityMetadata {
        unimplemented!()
    }

    /// Never used and is unimplemented
    pub fn read_particle(&mut self, _id: i32) -> Particle {
        unimplemented!()
    }

    /// Never used and is unimplemented
    pub fn read_player_info_action(&mut self) -> PlayerInfoAction {
        unimplemented!()
    }

    /// Reads a [`Component`](chat::Component) from a json string in this buffer
    pub fn read_chat(&mut self) -> Component {
        serde_json::from_str(&self.read_string()).expect("Error reading Component")
    }

    /// Writes a byte to this buffer, expanding the buffer if needed.
    #[inline]
    pub fn write(&mut self, byte: u8) {
        if self.cursor >= self.inner.len() {
            self.inner.push(byte);
        } else {
            self.inner[self.cursor] = byte;
        }
        self.cursor += 1;
    }

    /// Writes the given bytes to this buffer.
    #[inline]
    pub fn write_bytes(&mut self, blob: &[u8]) {
        self.ensure_size(self.cursor + blob.len());
        self.write_bytes_unchecked(blob);
    }

    /// Writes the given bytes to this buffer without performing size checks.
    #[inline]
    fn write_bytes_unchecked(&mut self, blob: &[u8]) {
        (self.inner[self.cursor .. self.cursor + blob.len()]).copy_from_slice(blob);
        self.cursor += blob.len();
    }

    /// Writes the given bool to this buffer as a byte with value `1` corresponding to true, and
    /// `0` corresponding to false.
    #[inline]
    pub fn write_bool(&mut self, value: bool) {
        if value {
            self.write(1);
        } else {
            self.write(0);
        }
    }

    /// An alias for the `write` method.
    #[inline]
    pub fn write_u8(&mut self, value: u8) {
        self.write(value);
    }

    /// Writes the given signed byte to this buffer.
    #[inline]
    pub fn write_i8(&mut self, value: i8) {
        self.write(value as u8);
    }

    /// Writes the given two-byte, unsigned integer to this buffer.
    #[inline]
    pub fn write_u16(&mut self, value: u16) {
        self.ensure_size(self.cursor + 2);
        self.inner[self.cursor] = (value >> 8) as u8;
        self.inner[self.cursor + 1] = value as u8;
        self.cursor += 2;
    }

    /// Writes the given two-byte, signed integer to this buffer.
    #[inline]
    pub fn write_i16(&mut self, value: i16) {
        self.write_u16(value as u16);
    }

    /// Writes the given four-byte, signed integer to this buffer.
    #[inline]
    pub fn write_i32(&mut self, value: i32) {
        self.ensure_size(self.cursor + 4);
        self.inner[self.cursor] = (value >> 24) as u8;
        self.inner[self.cursor + 1] = (value >> 16) as u8;
        self.inner[self.cursor + 2] = (value >> 8) as u8;
        self.inner[self.cursor + 3] = value as u8;
        self.cursor += 4;
    }

    /// Writes the given eight-byte, signed integer to this buffer.
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
        self.cursor += 8;
    }

    /// Writes the given 16-byte, unsigned integer to this buffer.
    #[inline]
    pub fn write_u128(&mut self, value: u128) {
        self.ensure_size(self.cursor + 16);
        self.inner[self.cursor] = (value >> 120) as u8;
        self.inner[self.cursor + 1] = (value >> 112) as u8;
        self.inner[self.cursor + 2] = (value >> 104) as u8;
        self.inner[self.cursor + 3] = (value >> 96) as u8;
        self.inner[self.cursor + 4] = (value >> 88) as u8;
        self.inner[self.cursor + 5] = (value >> 80) as u8;
        self.inner[self.cursor + 6] = (value >> 72) as u8;
        self.inner[self.cursor + 7] = (value >> 64) as u8;
        self.inner[self.cursor + 8] = (value >> 56) as u8;
        self.inner[self.cursor + 9] = (value >> 48) as u8;
        self.inner[self.cursor + 10] = (value >> 40) as u8;
        self.inner[self.cursor + 11] = (value >> 32) as u8;
        self.inner[self.cursor + 12] = (value >> 24) as u8;
        self.inner[self.cursor + 13] = (value >> 16) as u8;
        self.inner[self.cursor + 14] = (value >> 8) as u8;
        self.inner[self.cursor + 15] = value as u8;
        self.cursor += 16;
    }

    /// Writes the given 32-bit float to this buffer.
    #[inline]
    pub fn write_f32(&mut self, value: f32) {
        self.write_i32(value.to_bits() as i32);
    }

    /// Writes the given 64-bit float to this buffer.
    #[inline]
    pub fn write_f64(&mut self, value: f64) {
        self.write_i64(value.to_bits() as i64);
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

    /// Writes the given variable length integer to this buffer.
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

    /// Writes the given variable length long to this buffer.
    pub fn write_varlong(&mut self, mut value: i64) {
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

    /// Writes the given string to this buffer, prefixed by its length encoded as a variable lengthed
    /// integer.
    #[inline]
    pub fn write_string(&mut self, value: &String) {
        let bytes = value.as_bytes();
        self.write_varint(bytes.len() as i32);
        self.ensure_size(self.cursor + value.len());
        self.write_bytes_unchecked(bytes);
    }

    /// Writes the given byte array to this buffer.
    #[inline]
    pub fn write_byte_array(&mut self, value: &[u8]) {
        self.ensure_size(self.cursor + value.len());
        self.write_bytes_unchecked(value);
    }

    /// Writes the given UUID to this buffer as a 16-byte unsigned integer.
    #[inline]
    pub fn write_uuid(&mut self, value: Uuid) {
        self.write_u128(value.as_u128());
    }

    /// Writes all the elements in the array to the buffer using using the provided serializer method
    #[inline]
    pub fn write_array<T>(&mut self, value: &Vec<T>, serializer: fn(&mut Self, &T) -> ()) {
        for e in value {
            serializer(self, e);
        }
    }

    /// Writes all the elements in the array to the buffer using using the provided serializer method
    #[inline]
    pub fn write_primative_array<T: Copy>(
        &mut self,
        value: &Vec<T>,
        serializer: fn(&mut Self, T) -> (),
    )
    {
        for &e in value {
            serializer(self, e);
        }
    }

    /// Converts a [`BlockPosition`](crate::world::location::BlockPosition) to an i64 and writes it to this buffer
    #[inline]
    pub fn write_position(&mut self, value: &BlockPosition) {
        self.write_i64(
            ((value.x as i64 & 0x3FFFFFF) << 38)
                | ((value.z as i64 & 0x3FFFFFF) << 12)
                | (value.y as i64 & 0xFFF),
        );
    }

    /// Converts an [`UnlocalizedName`](util::UnlocalizedName) to a string and writes it to this buffer
    #[inline]
    pub fn write_unlocalized_name(&mut self, value: &UnlocalizedName) {
        self.write_string(&value.to_string());
    }

    /// Converts a [`NbtCompound`](nbt::NbtCompound) to bytes and writes it to this buffer
    /// This does not apply compression
    #[inline]
    pub fn write_nbt_tag(&mut self, value: &nbt::NbtCompound) {
        nbt::write::write_nbt_uncompressed(&mut self.inner, "root", value)
            .expect("Error writing nbt compound")
    }

    /// Writes a [`EntityMetadata`](super::network_handler::EntityMetadata) to this buffer
    pub fn write_entity_metadata(&mut self, value: &EntityMetadata) {
        match value {
            EntityMetadata::Byte(v) => self.write_i8(*v),
            EntityMetadata::VarInt(v) => self.write_varint(*v),
            EntityMetadata::Float(v) => self.write_f32(*v),
            EntityMetadata::String(v) => self.write_string(v),
            EntityMetadata::Chat(v) => self.write_chat(v),
            EntityMetadata::OptChat(b, c) => {
                self.write_bool(*b);
                match c {
                    Some(c) => self.write_chat(c),
                    None => {}
                }
            }
            EntityMetadata::Slot(v) => self.write_slot(v),
            EntityMetadata::Boolean(v) => self.write_bool(*v),
            EntityMetadata::Rotation(x, y, z) => {
                self.write_f32(*x);
                self.write_f32(*y);
                self.write_f32(*z);
            }
            EntityMetadata::Position(v) => self.write_position(v),
            EntityMetadata::OptPosition(b, p) => {
                self.write_bool(*b);
                match p {
                    Some(p) => self.write_position(p),
                    None => {}
                }
            }
            EntityMetadata::Direction(v) => self.write_varint(*v),
            EntityMetadata::OptUUID(b, u) => {
                self.write_bool(*b);
                match u {
                    Some(u) => self.write_uuid(*u),
                    None => {}
                }
            }
            EntityMetadata::OptBlockId(v) => self.write_varint(*v),
            EntityMetadata::NBT(v) => self.write_nbt_tag(v),
            EntityMetadata::Particle(v) => self.write_wrapped_particle(v),
            EntityMetadata::VillagerData(a, b, c) => {
                self.write_varint(*a);
                self.write_varint(*b);
                self.write_varint(*c);
            }
            EntityMetadata::OptVarInt(v) => self.write_varint(*v),
            EntityMetadata::Pose(v) => self.write_varint(*v),
        }
    }

    /// Writes a [`Particle`](super::packet_handler::Particle) to this buffer
    pub fn write_particle(&mut self, value: &Particle) {
        match value {
            Particle::Block(v) => self.write_varint(*v),
            Particle::Dust(x, y, z, s) => {
                self.write_f32(*x);
                self.write_f32(*y);
                self.write_f32(*z);
                self.write_f32(*s);
            }
            Particle::FallingDust(v) => self.write_varint(*v),
            Particle::Item(v) => self.write_slot(v),
            _ => {}
        }
    }

    /// Writes a [`PlayerInfoAction`](super::packet_handler::PlayerInfoAction) to this buffer
    pub fn write_player_info_action(&mut self, value: &PlayerInfoAction) {
        match value {
            PlayerInfoAction::AddPlayer {
                name,
                number_of_properties,
                properties,
                gamemode,
                ping,
                has_display_name,
                display_name,
            } => {
                self.write_string(name);
                self.write_varint(*number_of_properties);
                self.write_array(properties, Self::write_player_property);
                self.write_varint(*gamemode);
                self.write_varint(*ping);
                self.write_bool(*has_display_name);
                match display_name {
                    Some(v) => self.write_chat(v),
                    None => {}
                }
            }
            PlayerInfoAction::UpdateGamemode { gamemode } => {
                self.write_varint(*gamemode);
            }
            PlayerInfoAction::UpdateLatency { ping } => {
                self.write_varint(*ping);
            }
            PlayerInfoAction::UpdateDisplayName {
                has_display_name,
                display_name,
            } => {
                self.write_bool(*has_display_name);
                match display_name {
                    Some(v) => self.write_chat(v),
                    None => {}
                }
            }
            PlayerInfoAction::RemovePlayer => {}
        }
    }

    /// Converts a [`Component`](chat::Component) to a json string and writes it to this buffer
    pub fn write_chat(&mut self, value: &Component) {
        self.write_string(
            &serde_json::to_string(value).expect("Error converting a Component to a string"),
        );
    }
}
