use crate::{block::states::STATE_COUNT, util::math::fast_ceil_log2_64, StateID};
use std::num::NonZeroU8;

/// The minimum number of bits per block.
pub const MIN_BITS_PER_BLOCK: u8 = 4;
/// The maximum number of bits required to represent a block state.
pub const MAX_BITS_PER_BLOCK: u8 = fast_ceil_log2_64(STATE_COUNT as u64) as u8;
/// If the bits per block is greater than **or equal to** this value, then a direct palette should
/// be used over an indirect palette.
pub const DIRECT_PALETTE_THRESHOLD: u8 = 9;

pub struct Palette {
    pub(super) index_to_state: Vec<StateID>,
    // TODO: consider changing index type when/if StateID is changed
    state_to_index: Vec<(StateID, u16)>,
    bits_per_block: NonZeroU8,
}

impl Palette {
    pub const fn new() -> Self {
        Palette {
            index_to_state: Vec::new(),
            state_to_index: Vec::new(),
            // Safety: MIN_BITS_PER_BLOCK is not zero
            bits_per_block: unsafe { NonZeroU8::new_unchecked(MIN_BITS_PER_BLOCK) },
        }
    }

    pub fn singleton(state: StateID) -> Self {
        Palette {
            index_to_state: vec![state],
            state_to_index: vec![(state, 0)],
            bits_per_block: unsafe { NonZeroU8::new_unchecked(MIN_BITS_PER_BLOCK) },
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        debug_assert!(
            self.state_to_index.len() == self.index_to_state.len(),
            "State-to-index and index-to-state maps are not the same size"
        );

        self.state_to_index.len()
    }

    pub fn states(&self) -> impl Iterator<Item = StateID> + '_ {
        self.index_to_state.iter().copied()
    }

    /// Returns the number of bits that should be used for each state/index in the block data
    /// storage. This value is based off of the ceiled base-2 logarithm of the length of the
    /// palette. If the palette is empty, or the result of the logarithm is less than
    /// `MIN_BITS_PER_BLOCK`, then `MIN_BITS_PER_BLOCK` is returned. If the result of the logarithm
    /// is greater than or equal to `DIRECT_PALETTE_THRESHOLD`, then `MAX_BITS_PER_BLOCK` is
    /// returned and a direct palette should be used.
    #[inline]
    pub fn bits_per_block(&self) -> NonZeroU8 {
        self.bits_per_block
    }

    #[inline]
    fn bits_per_block_internal(len: usize) -> NonZeroU8 {
        // If len == 0, then this value will be 63
        let raw_bits_per_block = fast_ceil_log2_64(len as u64) as u8;

        if raw_bits_per_block < DIRECT_PALETTE_THRESHOLD {
            let bpb = u8::max(MIN_BITS_PER_BLOCK, raw_bits_per_block);

            // Safety: MIN_BITS_PER_BLOCK is greater than zero, and we take the max of it against
            // `raw_bits_per_block`
            unsafe { NonZeroU8::new_unchecked(bpb) }
        } else {
            // If the palette is empty, use the minimum number of bits
            if raw_bits_per_block == 63 {
                // Safety: MIN_BITS_PER_BLOCK is not zero
                unsafe { NonZeroU8::new_unchecked(MIN_BITS_PER_BLOCK) }
            }
            // Otherwise use the maximum number of bits
            else {
                // Safety: MAX_BITS_PER_BLOCK is not zero
                unsafe { NonZeroU8::new_unchecked(MAX_BITS_PER_BLOCK) }
            }
        }
    }

    // TODO: consider adding state_for_unchecked depending on how this API shakes out
    #[inline]
    pub fn state_for(&self, index: usize) -> Option<StateID> {
        self.index_to_state.get(index).copied()
    }

    #[inline]
    pub fn index_of(&self, state: StateID) -> Option<usize> {
        self.state_to_index
            .binary_search_by_key(&state, |&(s, _)| s)
            .ok()
            .map(|trans_index| self.state_to_index[trans_index].1 as usize)
    }

    pub fn insert(&mut self, state: StateID) -> InsertionResult {
        // Grab the index at which the state will be inserted in the state-to-index lookup map
        let insertion_index = match self
            .state_to_index
            .binary_search_by_key(&state, |&(s, _)| s)
        {
            Ok(index) => return InsertionResult::AlreadyInPalette { index },
            Err(index) => index,
        };

        // Store the actual index of the state in the palette
        let index = self.index_to_state.len();
        // Store the old length for checking if we need to update bits_per_block
        let old_len = self.len();

        // Add the state to each map
        self.index_to_state.push(state);
        self.state_to_index
            .insert(insertion_index, (state, index as u16));

        // If the current length is a power of two, then we may need to adjust bits_per_block
        let old_bits_per_block = if old_len.count_ones() == 1 {
            // Just because the length is a power of two doesn't mean the bits per block changed,
            // so we perform that more expensive additional check down here

            let current_bits_per_block = self.bits_per_block;
            let new_bits_per_block = Self::bits_per_block_internal(old_len + 1);

            if current_bits_per_block != new_bits_per_block {
                self.bits_per_block = new_bits_per_block;
                Some(current_bits_per_block)
            } else {
                None
            }
        } else {
            None
        };

        match old_bits_per_block {
            None => InsertionResult::Inserted { index },
            Some(old_bits_per_block) => InsertionResult::InsertedAndAltered {
                index,
                old_bits_per_block,
                new_bits_per_block: self.bits_per_block,
            },
        }
    }

    pub fn remove(&mut self, state: StateID) -> RemovalResult {
        // Grab the index of the element in the state-to-index map to remove
        let removal_index = match self
            .state_to_index
            .binary_search_by_key(&state, |&(s, _)| s)
        {
            Ok(index) => index,
            Err(_) => return RemovalResult::NotInPalette,
        };

        // Grab the index in the index-to-state map at which to remove the element
        let index = self.state_to_index[removal_index].1 as usize;

        self.index_to_state.remove(index);
        self.state_to_index.remove(removal_index);

        // Update the state-to-index map to account for the fact that we just removed a state
        self.state_to_index
            .iter_mut()
            .map(|(_, idx)| idx)
            .filter(|idx| **idx as usize > index)
            .for_each(|idx| *idx -= 1);

        // If our length is now a power of two, then we may need to update the bits per block
        let new_len = self.len();
        let old_bits_per_block = if new_len.count_ones() == 1 {
            let current_bits_per_block = self.bits_per_block;
            let new_bits_per_block = Self::bits_per_block_internal(new_len);

            if current_bits_per_block != new_bits_per_block {
                self.bits_per_block = new_bits_per_block;
                Some(current_bits_per_block)
            } else {
                None
            }
        } else {
            None
        };

        match old_bits_per_block {
            None => RemovalResult::Removed { index },
            Some(old_bits_per_block) => RemovalResult::RemovedAndAltered {
                index,
                old_bits_per_block,
                new_bits_per_block: self.bits_per_block,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertionResult {
    AlreadyInPalette {
        index: usize,
    },
    Inserted {
        index: usize,
    },
    InsertedAndAltered {
        index: usize,
        old_bits_per_block: NonZeroU8,
        new_bits_per_block: NonZeroU8,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemovalResult {
    NotInPalette,
    Removed {
        index: usize,
    },
    RemovedAndAltered {
        index: usize,
        old_bits_per_block: NonZeroU8,
        new_bits_per_block: NonZeroU8,
    },
}
