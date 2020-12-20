use crate::{
    block::entity::BlockEntity,
    item::{get_item, Inventory, ItemStack},
    world::location::BlockPosition,
};
use quartz_nbt::NbtCompound;
use util::UnlocalizedName;

// While this is somewhat accurate to how the Furnace BE will be implemented the tick method is no where near finished and some key fields are missing
// Currently this is mostly for testing BEs

pub struct FurnaceBlockEntity {
    pos: BlockPosition,
    custom_name: String,
    lock: bool,
    items: Inventory,
    burn_time: i32,
    cook_time: i32,
    cook_time_total: i32,
    active: bool,
}

impl FurnaceBlockEntity {
    pub fn new(pos: BlockPosition, name: Option<String>) -> Self {
        FurnaceBlockEntity {
            pos,
            custom_name: match name {
                Some(name) => name,
                _ => "Furnace".to_owned(),
            },
            lock: false,
            items: Inventory::new(3),
            burn_time: 0,
            cook_time: 0,
            cook_time_total: 0,
            active: false,
        }
    }
}

impl BlockEntity for FurnaceBlockEntity {
    fn from_nbt(&mut self, nbt: &NbtCompound) {
        self.burn_time = nbt.get("BurnTime").unwrap_or(0);
        self.cook_time = nbt.get("CookTime").unwrap_or(0);
        self.cook_time_total = nbt.get("CookTimeTotal").unwrap_or(0);
        self.items.from_tag(nbt);

        if nbt.contains_key("CustomName") {
            self.custom_name = nbt.get("CustomName").unwrap_or("Furnace").to_owned();
        }

        if nbt.contains_key("Lock") {
            self.lock = nbt.get("Lock").unwrap_or(false);
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.insert("BurnTime".to_owned(), self.burn_time);
        nbt.insert("CookTime".to_owned(), self.cook_time);
        nbt.insert("CookTimeTotal".to_owned(), self.cook_time_total);
        self.items.write_tag(nbt);
        nbt.insert("CustomName".to_owned(), self.custom_name.clone());
        nbt.insert("Lock".to_owned(), self.lock);
    }

    fn tick(&mut self) {
        // Currently just testing of inventories
        if self.active {
            self.cook_time += 1;
            if self.cook_time > self.cook_time_total {
                self.items.insert(
                    2,
                    ItemStack::new(get_item(&UnlocalizedName::minecraft("stone")).unwrap()),
                );
            }
        } else {
            if self.items.get(2).is_empty() {
                self.items.insert(
                    2,
                    ItemStack::new(get_item(&UnlocalizedName::minecraft("stone")).unwrap()),
                );
            } else {
                self.items.increment(2);
            }
        }
    }
}
