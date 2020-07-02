use nbt::{NbtCompound, NbtList};
use crate::item::item::{ItemStack, OptionalItemStack};

// Represents a basic inventory
pub struct Inventory {
    pub size: usize,
    items: Box<[OptionalItemStack]>
}

impl Inventory {
    pub fn new(size: usize) -> Self {
        Inventory {
            size,
            items: vec![OptionalItemStack::new(None); size].into_boxed_slice()
        }
    }

    // Assume index is always within bounds as items has a static size and all calls should be prefixed with a can_insert if slot number is not hard coded
    // Return the previous stack
    pub fn insert(&mut self, index: usize, item: ItemStack) -> OptionalItemStack {
        let current_item = self.items[index].clone();
        self.items[index] = OptionalItemStack::new(Some(item));
        current_item
    }

    pub fn increment(&mut self, index: usize) {
        self.items[index].item().unwrap().count += 1;
    }

    pub fn get(&self, index: usize) -> OptionalItemStack {
        self.items.get(index).unwrap().clone()
    }

    pub fn can_insert(&self, index: usize) -> bool {
        self.size > index
    }

    pub fn from_tag(&mut self, nbt: &NbtCompound) {
        let list = nbt.get_list("Items").unwrap();

        for i in 0..list.len() {
            // List has to have a element at every index because even slots without items need to have an empty stack
            let compound = list.get_compound(i).unwrap();
            let slot = compound.get_byte("Slot").unwrap_or(0) as usize;

            if slot < self.size {
                self.items[slot] = OptionalItemStack::new(Some(ItemStack::from_nbt(compound.clone())));
            }
        }
    }

    pub fn write_tag(&self,  tag: &mut NbtCompound) {
        let mut list = NbtList::new();

        for i in 0..self.size {
            let mut slot_tag = NbtCompound::new();

            slot_tag.set_byte("Slot".to_owned(), i as i8);
            // Every index must have a stack, even if it is empty
            let item = self.items.get(i).unwrap();
            item.write_nbt(&mut slot_tag);
            list.add_compound(slot_tag);
        }

        tag.set_list("Items".to_owned(), list);
    }
}