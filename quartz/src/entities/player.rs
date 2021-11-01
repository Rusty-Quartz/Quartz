use hecs::Bundle;
use qdat::Gamemode;

use crate::{
    entities::Position,
    item::{Inventory, OptionalItemStack, EMPTY_ITEM_STACK},
    network::AsyncWriteHandle,
};

#[derive(Bundle)]
pub struct Player {
    inventory: PlayerInventory,
    pos: Position,
    gamemode: Gamemode,
    write_handle: AsyncWriteHandle,
}

impl Player {
    pub fn new(gamemode: Gamemode, pos: Position, write_handle: AsyncWriteHandle) -> Self {
        Player {
            inventory: PlayerInventory::new(),
            pos,
            gamemode,
            write_handle,
        }
    }
}

pub struct PlayerInventory {
    current_slot: u8,
    inv: Inventory,
    offhand_slot: OptionalItemStack,
}

impl PlayerInventory {
    pub fn new() -> PlayerInventory {
        Self {
            current_slot: 36,
            inv: Inventory::new(46),
            offhand_slot: EMPTY_ITEM_STACK,
        }
    }

    pub fn set_curr_slot(&mut self, slot: u8) {
        self.current_slot = slot;
    }

    pub fn set_slot(&mut self, slot: usize, item: OptionalItemStack) -> OptionalItemStack {
        self.inv.insert(slot, item)
    }

    pub fn swap_hands(&mut self) {
        self.offhand_slot = self.inv.insert(
            self.current_slot as usize,
            std::mem::take(&mut self.offhand_slot),
        );
    }

    pub fn current_slot(&self) -> OptionalItemStack {
        self.inv.get(self.current_slot as usize)
    }

    pub fn swap_slots(&mut self, a: usize, b: usize) {
        self.inv.swap(a, b);
    }
}
