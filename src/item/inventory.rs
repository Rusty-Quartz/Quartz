use crate::nbt::{NbtCompound, NbtList};
use crate::item::item::ItemStack;

pub struct Inventory {
    pub size: usize,
    items: Box<[ItemStack]>
}

impl Inventory {
    pub fn new(size: usize) -> Self {
        Inventory {
            size,
            items: vec![ItemStack::empty(); size].into_boxed_slice()
        }
    }

    pub fn insert(&mut self, index: usize, item: ItemStack) -> ItemStack {
        let current_item = self.items[index].clone();
        self.items[index] = item;
        current_item
    }

    pub fn increment(&mut self, index: usize) {
        self.items[index].count += 1;
    }

    pub fn get(&self, index: usize) -> &ItemStack {
        self.items.get(index).unwrap()
    }

    pub fn can_insert(&self, index: usize) -> bool {
        self.size > index
    }

    pub fn from_tag(&mut self, nbt: &NbtCompound) {
        let list = nbt.get_list("Items").unwrap();

        for i in 0..list.len() {
            let compound = list.get_compound(i).unwrap();
            let slot = compound.get_byte("Slot") as usize;

            if slot < self.size {
                self.items[slot] = ItemStack::from_nbt(compound.clone());
            }
        }
    }

    pub fn write_tag(&self,  tag: &mut NbtCompound) {
        let mut list = NbtList::new();

        for i in 0..self.size {
            let mut slot_tag = NbtCompound::new();

            slot_tag.set_byte("Slot".to_owned(), i as i8);
            let item = self.items.get(i).unwrap();
            item.write_nbt(&mut slot_tag);
            list.add_compound(slot_tag);
        }

        tag.set_list("Items".to_owned(), list);
    }
}