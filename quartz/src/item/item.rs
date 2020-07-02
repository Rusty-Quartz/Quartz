use mcutil::UnlocalizedName;
use nbt::NbtCompound;
use crate::item::get_item;
use crate::item::ItemInfo;

#[derive(Debug)]
pub struct Item {
    pub id: UnlocalizedName,
    pub stack_size: u8,
    pub rarity: u8,
    pub item_info: Option<ItemInfo>
}

#[derive(Clone)]
pub struct ItemStack {
    pub item: &'static Item,
    pub count: u8,
    pub damage: u32,
    pub nbt: NbtCompound
}

impl ItemStack {

    pub fn empty() -> Self {
        ItemStack {
            item: get_item(&UnlocalizedName::minecraft("air")).expect("Item list not initialized"),
            count: 0,
            damage: 0,
            nbt: NbtCompound::new()
        }
    }

    pub fn new(item: &'static Item) -> Self {
        ItemStack {
            item: item,
            count: 1,
            damage: if item.item_info.is_some() {
                item.item_info.as_ref().unwrap().max_durability()
            } else { 0 },
            nbt: NbtCompound::new()
        }
    }

    // Write stack to nbt tag
    pub fn write_nbt(&self, tag: &mut NbtCompound) {
        tag.set_byte("Count".to_owned(), self.count as i8);
        tag.set_byte("Damage".to_owned(), self.damage as i8);
        tag.set_string("id".to_owned(), self.item.id.to_string());
        tag.set_compound("tag".to_owned(), self.nbt.clone());
    }

    pub fn from_nbt(tag: NbtCompound) -> Self {
        let tag = match tag.has("tag") {
            true => match tag.get_compound("tag") {
                Some(tag) => tag.clone().to_owned(),
                _ => NbtCompound::new()
            },
            _ => NbtCompound::new()
        };

        let damage = if tag.has("Damage") { tag.get_int("Damage").unwrap_or(0) } else { 0 } as u32;

        ItemStack {
            item: get_item(&UnlocalizedName::parse(tag.get_string("id").unwrap_or("minecraft:air")).unwrap()).unwrap(),
            count: tag.get_byte("Count").unwrap_or(0) as u8,
            damage,
            nbt: tag
        }
    }

    // Any empty stack is any stack that has a count of 0 or is air
    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item.id == UnlocalizedName::minecraft("air")
    }
}

#[repr(transparent)]
#[derive(Clone)]
pub struct OptionalItemStack(Option<Box<ItemStack>>);

impl OptionalItemStack {
    pub fn new(stack: Option<ItemStack>) -> Self {
        if stack.is_none() {
            return OptionalItemStack(None)
        }
        OptionalItemStack(Some(Box::new(stack.unwrap())))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none() || self.0.clone().unwrap().is_empty()
    }

    pub fn write_nbt(&self, tag: &mut NbtCompound) {
        if !self.0.is_none() {
            self.0.clone().unwrap().write_nbt(tag)
        }    
    }

    pub fn from_nbt(tag: NbtCompound) -> Self {
        OptionalItemStack(Some(Box::new(ItemStack::from_nbt(tag))))
    }

    pub fn item(&self) -> Option<Box<ItemStack>> {
        self.0.clone()
    }
}