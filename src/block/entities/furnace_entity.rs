use crate::block::entity::{BlockEntity, BlockEntityType};
use crate::nbt::{NbtCompound};
use crate::data::UnlocalizedName;
use crate::data::BlockPosition;
use crate::item::{get_item, ItemStack, Item};
use crate::item::inventory::Inventory;

pub struct FurnaceBlockEntity {
    pos: BlockPosition,
    custom_name: String,
    lock: bool,
    items: Inventory,
    burn_time: i32,
    cook_time: i32,
    cook_time_total: i32,
    active: bool
}

impl FurnaceBlockEntity {
    pub fn new(pos: BlockPosition, name: Option<String>) -> Self {
        FurnaceBlockEntity {
            pos,
            custom_name: match name {
                Some(name) => name,
                _ => "Furnace".to_owned()
            },
            lock: false,
            items: Inventory::new(3),
            burn_time: 0,
            cook_time: 0,
            cook_time_total: 0,
            active: false
        }
    }
}

impl BlockEntity for FurnaceBlockEntity {
    fn from_nbt(&mut self, nbt: &NbtCompound) {
        self.burn_time = nbt.get_int("BurnTime");
        self.cook_time = nbt.get_int("CookTime");
        self.cook_time_total = nbt.get_int("CookTimeTotal");
        self.items.from_tag(nbt);
        
        if nbt.has("CustomName") {
            self.custom_name = nbt.get_string("CustomName").to_owned();
        }

        if nbt.has("Lock") {
            self.lock = nbt.get_bool("Lock");
        }
    }

    fn write_nbt(&mut self, nbt: &mut NbtCompound) {
        nbt.set_int("BurnTime".to_owned(), self.burn_time);
        nbt.set_int("CookTime".to_owned(), self.cook_time);
        nbt.set_int("CookTimeTotal".to_owned(), self.cook_time_total);
        self.items.write_tag(nbt);
        nbt.set_string("CustomName".to_owned(), self.custom_name.clone());
        nbt.set_bool("Lock".to_owned(), self.lock);
    }

    fn tick(&mut self) {
        if self.active {
            self.cook_time += 1;
            if self.cook_time > self.cook_time_total {
                self.items.insert(2, ItemStack::new( get_item(&UnlocalizedName::minecraft("stone")).unwrap()));
            }
		}
		else {
			if self.items.get(2).is_empty() {
                self.items.insert(2, ItemStack::new(get_item(&UnlocalizedName::minecraft("stone")).unwrap()));
            }
            else {
                self.items.increment(2);
            }
		}
    }

    fn id(&self) -> BlockEntityType {
        BlockEntityType::Furnace
    }    
}