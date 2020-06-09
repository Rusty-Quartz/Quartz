use crate::util::UnlocalizedName;
use crate::nbt::NbtCompound;
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

    // Is used for filling inventories
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

    pub fn from_nbt(nbt: NbtCompound) -> Self {
        let tag = match nbt.has("tag") {
            true => match nbt.get_compound("tag") {
                Some(tag) => tag.clone().to_owned(),
                _ => NbtCompound::new()
            },
            _ => NbtCompound::new()
        };

        let damage = if tag.has("Damage") { tag.get_int("Damage") } else { 0 } as u32;

        ItemStack {
            item: get_item(&UnlocalizedName::parse(nbt.get_string("id")).unwrap()).unwrap(),
            count: nbt.get_byte("Count") as u8,
            damage,
            nbt: tag
        }
    }

    // Any empty stack is any stack that has a count of 0 or is air
    pub fn is_empty(&self) -> bool {
        self.count <= 0 || self.item.id == UnlocalizedName::minecraft("air")
    }
}