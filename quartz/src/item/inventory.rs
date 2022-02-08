use crate::item::{ItemStack, OptionalItemStack};
use quartz_nbt::{NbtCompound, NbtList};

/// Represents a basic inventory
#[derive(Clone)]
pub struct Inventory {
    /// The size of the inventory
    pub size: usize,
    /// The items in the inventory
    items: Box<[OptionalItemStack]>,
}

impl Inventory {
    /// Creates a new inventory with a specified size
    pub fn new(size: usize) -> Self {
        Inventory {
            size,
            items: vec![OptionalItemStack::new(None); size].into_boxed_slice(),
        }
    }

    // Assume index is always within bounds as items has a static size and all calls should be prefixed with a can_insert if slot number is not hard coded
    // Return the previous stack
    /// Inserts an ItemStack into the given slot
    /// # Panics
    /// Panics if the index is out of range for the inventory, all calls should be prefixed with a can_insert call
    pub fn insert(&mut self, index: usize, item: OptionalItemStack) -> OptionalItemStack {
        let current_item = self.items[index].clone();
        self.items[index] = item;
        current_item
    }

    /// Increments the amount in a slot
    /// # Panics
    /// Panics if the index is out of range for the inventory, all calls should be prefixed with a can_insert call
    pub fn increment(&mut self, index: usize) {
        self.items[index].item().unwrap().count += 1;
    }

    /// Gets a clone of the OptionalItemStack in a slot
    /// # Panics
    /// Panics if the index is out of range for the inventory, all calls should be prefixed with a can_insert call
    pub fn get(&self, index: usize) -> OptionalItemStack {
        self.items.get(index).unwrap().clone()
    }

    /// Tests if a slot can be inserted into
    pub const fn can_insert(&self, index: usize) -> bool {
        self.size > index && index > 0
    }

    /// Swaps the items in indecies `a` and `b`
    pub fn swap(&mut self, a: usize, b: usize) {
        self.items.swap(a, b);
    }

    /// Creates a new Inventory from a NbtCompound
    ///
    /// # NBT Format
    /// ```
    /// {Items: [{
    ///     Slot: Byte,
    ///     id: String,
    ///     Count: Byte
    ///     tag: Compound,
    /// }]}
    /// ```
    pub fn from_tag(&mut self, nbt: &NbtCompound) {
        let list = nbt.get::<_, &NbtList>("Items").unwrap();

        for i in 0 .. list.len() {
            // List has to have a element at every index because even slots without items need to have an empty stack
            let compound = list.get::<&NbtCompound>(i).unwrap();
            let slot = compound.get::<_, i32>("Slot").unwrap_or(0) as usize;

            if slot < self.size {
                self.items[slot] =
                    OptionalItemStack::new(Some(ItemStack::from_nbt(compound.clone())));
            }
        }
    }

    /// Writes the Inventory to a NbtCompound
    ///
    /// # NBT Format
    /// ```
    /// {Items: [{
    ///     Slot: Byte,
    ///     id: String,
    ///     Count: Byte
    ///     tag: Compound,
    /// }]}
    /// ```
    pub fn write_tag(&self, tag: &mut NbtCompound) {
        let mut list = NbtList::new();

        for i in 0 .. self.size {
            let mut slot_tag = NbtCompound::new();

            slot_tag.insert("Slot".to_owned(), i as i8);
            // Every index must have a stack, even if it is empty
            let item = self.items.get(i).unwrap();
            item.write_nbt(&mut slot_tag);
            list.push(slot_tag);
        }

        tag.insert("Items".to_owned(), list);
    }
}
