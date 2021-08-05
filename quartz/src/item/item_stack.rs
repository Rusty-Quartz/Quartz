use qdat::{item::Item, UlnStr};
use quartz_nbt::NbtCompound;

use super::get_item;

/// Represents a stack of items
#[derive(Clone)]
pub struct ItemStack {
    /// The item in the stack
    pub item: &'static Item,
    /// The size of the stack
    pub count: u8,
    /// The damage of the itemstack
    pub damage: u32,
    /// Extra nbt info about the stack
    pub nbt: NbtCompound,
}

impl ItemStack {
    /// Represents a empty item stack
    pub fn empty() -> Self {
        ItemStack {
            item: get_item(UlnStr::minecraft("air")).expect("Item list not initialized"),
            count: 0,
            damage: 0,
            nbt: NbtCompound::new(),
        }
    }

    /// Creates a new itemstack
    pub fn new(item: &'static Item) -> Self {
        ItemStack {
            item,
            count: 1,
            damage: if item.item_info.is_some() {
                item.item_info.as_ref().unwrap().max_durability()
            } else {
                0
            },
            nbt: NbtCompound::new(),
        }
    }

    /// Write the stack to nbt tag
    ///
    /// # NBT Format
    /// ```
    /// {
    ///     id: String,
    ///     Count: byte
    ///     tag: Compound,
    /// }
    /// ```
    /// For `tag` format check https://minecraft.gamepedia.com/Player.dat_format#Item_structure
    pub fn write_nbt(&self, tag: &mut NbtCompound) {
        tag.insert("Count".to_owned(), self.count as i8);
        tag.insert("Damage".to_owned(), self.damage as i8);
        tag.insert("id".to_owned(), self.item.id.to_string());
        tag.insert("tag".to_owned(), self.nbt.clone());
    }

    /// Create an ItemStack from a nbt tag
    ///
    /// # NBT Format
    /// ```
    /// {
    ///     id: String,
    ///     Count: byte
    ///     tag: Compound,
    /// }
    /// ```
    /// For `tag` format check https://minecraft.gamepedia.com/Player.dat_format#Item_structure
    pub fn from_nbt(tag: NbtCompound) -> Self {
        let tag = match tag.contains_key("tag") {
            true => match tag.get::<_, &NbtCompound>("tag") {
                Ok(tag) => tag.clone().to_owned(),
                _ => NbtCompound::new(),
            },
            _ => NbtCompound::new(),
        };

        let damage = if tag.contains_key("Damage") {
            tag.get::<_, i32>("Damage").unwrap_or(0)
        } else {
            0
        } as u32;

        ItemStack {
            item: get_item(UlnStr::from_str(tag.get("id").unwrap_or("minecraft:air")).unwrap())
                .unwrap(),
            count: tag.get::<_, i32>("Count").unwrap_or(0) as u8,
            damage,
            nbt: tag,
        }
    }

    /// Returns if the current stack is empty or not
    /// Any empty stack is any stack that has a count of 0 or is air
    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item.id == UlnStr::minecraft("air")
    }
}

/// An ItemStack wrapped in an Option to save memory when it is empty
#[repr(transparent)]
#[derive(Clone)]
pub struct OptionalItemStack(Option<Box<ItemStack>>);

impl OptionalItemStack {
    /// Creates a new OptionalItemStack
    pub fn new(stack: Option<ItemStack>) -> Self {
        if stack.is_none() {
            return OptionalItemStack(None);
        }
        OptionalItemStack(Some(Box::new(stack.unwrap())))
    }

    /// Is the OptionalItemStack empty / existant
    pub fn is_empty(&self) -> bool {
        self.0.is_none() || self.0.clone().unwrap().is_empty()
    }

    /// Writes the stack to an nbt tag
    // TODO: make sure this works when reading / writing the world files
    pub fn write_nbt(&self, tag: &mut NbtCompound) {
        if !self.0.is_none() {
            self.0.clone().unwrap().write_nbt(tag)
        }
    }

    /// Creates a new OptionalItemStack from a nbt tag
    pub fn from_nbt(tag: NbtCompound) -> Self {
        OptionalItemStack(Some(Box::new(ItemStack::from_nbt(tag))))
    }

    /// Gets the inner data of the OptionalItemStack
    pub fn item(&self) -> Option<Box<ItemStack>> {
        self.0.clone()
    }
}
