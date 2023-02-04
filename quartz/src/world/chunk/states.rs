use super::{Palette, MAX_BITS_PER_BLOCK, MIN_BITS_PER_BLOCK};
use crate::StateID;
use qdat::block::states::{is_air, AIR, CAVE_AIR, VOID_AIR};
use static_assertions::const_assert;
use std::{
    error::Error,
    fmt::{self, Display, Formatter},
    num::{NonZeroU8, NonZeroUsize},
};

#[derive(Clone)]
pub struct CompactStateBuffer {
    data: Vec<u64>,
    long_index: usize,
    bit_index: u8,
    meta: BufferMetadata,
}

impl CompactStateBuffer {
    pub fn empty() -> Self {
        CompactStateBuffer {
            data: Vec::new(),
            long_index: 0,
            bit_index: 0,
            meta: BufferMetadata::new(
                NonZeroU8::new(MIN_BITS_PER_BLOCK).expect("MIN_BITS_PER_BLOCK should not be zero"),
            ),
        }
    }

    pub fn new(data: Vec<u64>, bits_per_entry: NonZeroU8) -> Self {
        assert!(
            bits_per_entry.get() <= 64,
            "`bits_per_entry` cannot be greater than 64"
        );

        CompactStateBuffer {
            data,
            long_index: 0,
            bit_index: 0,
            meta: BufferMetadata::new(bits_per_entry),
        }
    }

    #[inline]
    pub const fn required_capacity(bits_per_entry: u8) -> usize {
        (4096 * bits_per_entry as usize) / 64
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    #[inline]
    pub fn inner(&self) -> &[u64] {
        &self.data
    }

    #[inline]
    pub fn into_inner(self) -> Vec<u64> {
        self.data
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.data
            .truncate(Self::required_capacity(self.meta.bits_per_entry.get()));
    }

    #[inline]
    pub fn reset_cursor(&mut self) {
        self.long_index = 0;
        self.bit_index = 0;
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        CompactStateBufferIter::new(self)
    }

    #[inline]
    pub fn advance_one(&mut self) {
        advance_one_internal(
            &mut self.long_index,
            &mut self.bit_index,
            self.meta.bits_per_entry,
            self.meta.data_bits_per_long,
        );
    }

    #[inline]
    pub fn advance(&mut self, entries: usize) {
        let entries_per_long = NonZeroUsize::from(self.meta.entries_per_long);
        self.long_index += entries / entries_per_long;
        self.bit_index += (entries % entries_per_long) as u8 * self.meta.bits_per_entry.get();
    }

    #[inline]
    pub fn jump_to(&mut self, nth: usize) {
        let (long_index, bit_index) = self.index_nth_entry(nth);
        self.long_index = long_index;
        self.bit_index = bit_index;
    }

    pub fn read_entry(&mut self) -> Option<usize> {
        let entry = self.entry_at(self.long_index, self.bit_index);
        if entry.is_some() {
            self.advance_one();
        }
        entry
    }

    pub fn peek_entry(&self) -> Option<usize> {
        self.entry_at(self.long_index, self.bit_index)
    }

    #[inline]
    pub fn nth_entry(&self, n: usize) -> Option<usize> {
        let (long_index, bit_index) = self.index_nth_entry(n);
        self.entry_at(long_index, bit_index)
    }

    pub fn write_entry(&mut self, entry: usize) {
        write_entry_to_vec(
            &mut self.data,
            self.long_index,
            self.bit_index,
            entry,
            self.meta.bits_per_entry,
        );
        self.advance_one();
    }

    #[inline]
    pub fn set_nth_entry(&mut self, n: usize, entry: usize) -> bool {
        let (long_index, bit_index) = self.index_nth_entry(n);
        write_entry_to(
            self.data.as_mut(),
            long_index,
            bit_index,
            entry,
            self.meta.bits_per_entry,
        )
    }

    pub fn block_count(&self, palette: Option<&Palette>) -> usize {
        match palette {
            Some(palette) => self.block_count_indirect(palette),
            None => self.block_count_direct(),
        }
    }

    fn block_count_indirect(&self, palette: &Palette) -> usize {
        let air = palette.index_of(AIR);
        let void_air = palette.index_of(VOID_AIR);
        let cave_air = palette.index_of(CAVE_AIR);
        let bpb = self.meta.bits_per_entry.get() as u32;

        let index = match (air, void_air, cave_air) {
            (None, None, None) => return 4096,
            (Some(index), None, None) => index,
            (None, Some(index), None) => index,
            (None, None, Some(index)) => index,
            // There is no quick check, so we do it the long way
            _ => {
                return self
                    .iter()
                    .flat_map(|index| palette.state_for(index))
                    .filter(|&state| !is_air(state))
                    .count();
            }
        };

        let index = index as u64;

        let all_air = if index == 0 {
            0
        } else {
            let mut long = 0;

            for shift in 0 .. 1 << MIN_BITS_PER_BLOCK {
                long |= index.overflowing_shl(shift * bpb).0;
            }

            long
        };

        let mut long_index = 0;
        let mut total_count = 0u32;
        let mut block_count = 0;

        let entries_per_long = self.meta.entries_per_long.get() as u32;
        let mask = self.meta.mask;

        while let Some(mut long) = self.data.get(long_index).copied() {
            // Quick check
            if long == all_air {
                long_index += 1;
                total_count += entries_per_long;
                continue;
            }

            // Individually check each entry
            for _ in 0 .. entries_per_long.min(4096 - total_count) {
                let entry = long & mask;
                if entry != index {
                    block_count += 1;
                }
                total_count += 1;
                long >>= bpb;
            }

            long_index += 1;
        }

        block_count
    }

    fn block_count_direct(&self) -> usize {
        let mut long_index = 0;
        let mut block_count = 0;

        const ENTRIES_PER_LONG: u32 = 64 / MAX_BITS_PER_BLOCK as u32;
        const MASK: u64 = (1u64 << MAX_BITS_PER_BLOCK) - 1;

        // If this fails then a `total_count` variable must be added like in the indirect block
        // count function
        const_assert!(4096 % ENTRIES_PER_LONG == 0);

        const fn gen_niche(id: StateID) -> u64 {
            let mut long = 0;
            let id = id as u64;
            let mut i = 0;
            while i < ENTRIES_PER_LONG {
                long |= id << (i as u8 * MAX_BITS_PER_BLOCK);
                i += 1;
            }
            long
        }

        const ALL_AIR: u64 = gen_niche(AIR);
        const ALL_VOID_AIR: u64 = gen_niche(VOID_AIR);
        const ALL_CAVE_AIR: u64 = gen_niche(CAVE_AIR);

        while let Some(mut long) = self.data.get(long_index).copied() {
            // Quick check
            if long == ALL_AIR || long == ALL_VOID_AIR || long == ALL_CAVE_AIR {
                long_index += 1;
                continue;
            }

            // Individually check each entry. We can get away with excluding a `total_count`
            // because ENTRIES_PER_LONG is divisible by
            for _ in 0 .. ENTRIES_PER_LONG {
                let entry = long & MASK;
                if !is_air(entry as StateID) {
                    block_count += 1;
                }
                long >>= MAX_BITS_PER_BLOCK;
            }

            long_index += 1;
        }

        block_count
    }

    pub fn to_direct_palette(&mut self, palette: &Palette) -> Result<(), PaletteConversionError> {
        let mut direct = CompactStateBuffer::new(
            vec![0; Self::required_capacity(MAX_BITS_PER_BLOCK)],
            NonZeroU8::new(MAX_BITS_PER_BLOCK).expect("MAX_BITS_PER_BLOCK should not be zero"),
        );

        let mut long_index = 0;
        let mut bit_index = 0;

        while let Some(entry) = self.entry_at(long_index, bit_index) {
            advance_one_internal(
                &mut long_index,
                &mut bit_index,
                self.meta.bits_per_entry,
                self.meta.data_bits_per_long,
            );

            let state = palette
                .state_for(entry)
                .ok_or(PaletteConversionError::IndexOutOfRange)?;
            direct.write_entry(state as usize);
        }

        direct.reset_cursor();
        *self = direct;
        Ok(())
    }

    pub fn to_indirect_palette(&mut self, palette: &Palette) -> Result<(), PaletteConversionError> {
        let old_bits_per_entry = self.meta.bits_per_entry.get();
        let new_bits_per_entry = palette.bits_per_block();

        if new_bits_per_entry.get() >= old_bits_per_entry {
            return Err(PaletteConversionError::PaletteTooLarge);
        }

        let old_entries_per_long = self.meta.entries_per_long.get();
        let old_mask = self.meta.mask;

        self.meta = BufferMetadata::new(new_bits_per_entry);
        self.reset_cursor();

        let mut read_long_index = 0;

        let mut last_read_state = StateID::MAX;
        let mut last_mapped_state: Option<usize> = None;

        // This closure allows us to optimize runs of states of the same type
        let mut map_state = |state: StateID| {
            if last_read_state != state {
                last_read_state = state;
                last_mapped_state = palette.index_of(state);
            }
            last_mapped_state
        };

        // We know that the following loop won't have any data races because the new bits_per_entry
        // value is less than the old value

        while let Some(mut long) = self.data.get(read_long_index).copied() {
            read_long_index += 1;

            for _ in 0 .. old_entries_per_long {
                let state = (long & old_mask) as u16;
                long >>= old_bits_per_entry;

                match map_state(state) {
                    Some(index) => self.write_entry(index),
                    None => return Err(PaletteConversionError::MissingState),
                }
            }
        }

        self.reset_cursor();
        self.shrink_to_fit();
        Ok(())
    }

    pub fn alter<F>(&mut self, mut f: F)
    where F: FnMut(usize) -> Option<usize> {
        let mut long_index = 0;
        let mut bit_index = 0;

        while let Some(entry) = self.entry_at(long_index, bit_index) {
            if let Some(altered) = f(entry) {
                write_entry_to(
                    self.data.as_mut(),
                    long_index,
                    bit_index,
                    altered,
                    self.meta.bits_per_entry,
                );
            }

            advance_one_internal(
                &mut long_index,
                &mut bit_index,
                self.meta.bits_per_entry,
                self.meta.data_bits_per_long,
            );
        }
    }

    #[inline]
    pub fn modify_bits_per_entry(&mut self, new_bits_per_entry: NonZeroU8) {
        match self.meta.bits_per_entry.cmp(&new_bits_per_entry) {
            std::cmp::Ordering::Greater => self.modify_bpe_in_place(new_bits_per_entry),
            std::cmp::Ordering::Less => self.modify_bpe_allocating(new_bits_per_entry),
            _ => {}
        }

        // If the new value is equal to the current value, do nothing
    }

    fn modify_bpe_allocating(&mut self, new_bits_per_entry: NonZeroU8) {
        // The change in bits per entry doesn't matter when allocating
        let mut modified = CompactStateBuffer::new(
            vec![0; Self::required_capacity(new_bits_per_entry.get())],
            new_bits_per_entry,
        );

        for mut long in self.data.iter().copied() {
            for _ in 0 .. self.meta.entries_per_long.get() {
                let entry = (long & self.meta.mask) as usize;
                long >>= self.meta.bits_per_entry.get();
                modified.write_entry(entry);
            }
        }

        modified.reset_cursor();
        *self = modified;
    }

    fn modify_bpe_in_place(&mut self, new_bits_per_entry: NonZeroU8) {
        // This only works if new BPE is less than old BPE
        assert!(
            new_bits_per_entry < self.meta.bits_per_entry,
            "cannot modify bits per entry in-place unless the new bits per entry value is less \
             than the current value"
        );

        let old_bits_per_entry = self.meta.bits_per_entry.get();
        let old_entries_per_long = self.meta.entries_per_long.get();
        let old_mask = self.meta.mask;

        self.meta = BufferMetadata::new(new_bits_per_entry);
        self.reset_cursor();

        let mut read_long_index = 0;

        while let Some(mut long) = self.data.get(read_long_index).copied() {
            read_long_index += 1;

            for _ in 0 .. old_entries_per_long {
                let entry = (long & old_mask) as usize;
                long >>= old_bits_per_entry;
                self.write_entry(entry);
            }
        }

        self.reset_cursor();
        self.shrink_to_fit();
    }

    #[inline]
    fn index_nth_entry(&self, n: usize) -> (usize, u8) {
        let long_index = n / NonZeroUsize::from(self.meta.entries_per_long);
        let bit_index = (n - long_index * self.meta.entries_per_long.get() as usize) as u8
            * self.meta.bits_per_entry.get();

        (long_index, bit_index)
    }

    #[inline]
    fn entry_at(&self, long_index: usize, bit_index: u8) -> Option<usize> {
        let entry = (*self.data.get(long_index)? >> bit_index) & self.meta.mask;
        Some(entry as usize)
    }
}

struct CompactStateBufferIter<'a> {
    buffer: &'a CompactStateBuffer,
    long_index: usize,
    bit_index: u8,
    count: u32,
}

impl<'a> CompactStateBufferIter<'a> {
    fn new(buffer: &'a CompactStateBuffer) -> Self {
        Self {
            buffer,
            long_index: 0,
            bit_index: 0,
            count: 0,
        }
    }
}

// TODO: add optimized impls for other functions
impl Iterator for CompactStateBufferIter<'_> {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.buffer.entry_at(self.long_index, self.bit_index);
        if entry.is_some() {
            if self.count >= 4096 {
                return None;
            }
            self.count += 1;

            advance_one_internal(
                &mut self.long_index,
                &mut self.bit_index,
                self.buffer.meta.bits_per_entry,
                self.buffer.meta.data_bits_per_long,
            );
        }
        entry
    }
}

#[inline]
fn advance_one_internal(
    long_index: &mut usize,
    bit_index: &mut u8,
    bits_per_entry: NonZeroU8,
    data_bits_per_long: NonZeroU8,
) {
    if *bit_index + bits_per_entry.get() < data_bits_per_long.get() {
        *bit_index += bits_per_entry.get();
    } else {
        *long_index += 1;
        *bit_index = 0;
    }
}

fn write_entry_to(
    dest: &mut [u64],
    long_index: usize,
    bit_index: u8,
    entry: usize,
    bits_per_entry: NonZeroU8,
) -> bool {
    debug_assert!(
        entry < (1 << bits_per_entry.get()),
        "Index must be able to fit in allocated bits"
    );

    // Make it look like we're using bits_per_entry even though it's just in a debug assert
    let _ = bits_per_entry;

    let insertion = (entry as u64) << bit_index;
    match dest.get_mut(long_index) {
        Some(long) => {
            // We have to clear the bits before we write the insertion;
            *long &= !((u64::MAX >> (u64::BITS as u64 - bits_per_entry.get() as u64)) << bit_index);
            *long |= insertion;
        }
        None => return false,
    }

    true
}

fn write_entry_to_vec(
    dest: &mut Vec<u64>,
    long_index: usize,
    bit_index: u8,
    entry: usize,
    bits_per_entry: NonZeroU8,
) {
    if long_index >= dest.len() {
        dest.push(0);
    }

    let result = write_entry_to(
        dest.as_mut_slice(),
        long_index,
        bit_index,
        entry,
        bits_per_entry,
    );
    debug_assert!(
        result,
        "Failed to expand vec to required capacity in compact state buffer."
    );
}

#[derive(Clone, Copy, Debug)]
pub enum PaletteConversionError {
    IndexOutOfRange,
    MissingState,
    PaletteTooLarge,
}

impl Display for PaletteConversionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::IndexOutOfRange => write!(f, "encountered index which was out of palette range"),
            Self::MissingState => write!(f, "encountered missing state in palette"),
            Self::PaletteTooLarge => write!(
                f,
                "palette bits per block is greater than buffer bits per entry"
            ),
        }
    }
}

impl Error for PaletteConversionError {}

#[derive(Clone, Copy, Debug)]
struct BufferMetadata {
    mask: u64,
    bits_per_entry: NonZeroU8,
    data_bits_per_long: NonZeroU8,
    entries_per_long: NonZeroU8,
}

impl BufferMetadata {
    #[inline]
    fn new(bits_per_entry: NonZeroU8) -> Self {
        assert!(
            bits_per_entry.get() <= 64,
            "`bits_per_entry` cannot be greater than 64"
        );

        BufferMetadata {
            mask: (1u64 << bits_per_entry.get()) - 1,
            bits_per_entry,
            // Safety: guaranteed by the assertion above
            data_bits_per_long: unsafe { NonZeroU8::new_unchecked(64 - (64 % bits_per_entry)) },
            // Safety: see above
            entries_per_long: unsafe { NonZeroU8::new_unchecked(64 / bits_per_entry) },
        }
    }
}
