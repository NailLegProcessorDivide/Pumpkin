use std::any::Any;
use std::sync::Arc;

use parking_lot::Mutex;
use pumpkin_world::{inventory::split_stack, item::ItemStack};

use pumpkin_world::inventory::{Clearable, Inventory};

use super::recipes::RecipeInputInventory;

#[derive(Debug, Clone)]
pub struct CraftingInventory {
    pub width: u8,
    pub height: u8,
    pub items: Vec<Arc<Mutex<ItemStack>>>,
}

impl CraftingInventory {
    pub fn new(width: u8, height: u8) -> Self {
        Self {
            width,
            height,
            items: {
                // Creates a Vec with different Mutexes for each slot
                let mut v = Vec::with_capacity(width as usize * height as usize);
                (0..width as usize * height as usize)
                    .for_each(|_| v.push(Arc::new(Mutex::new(ItemStack::EMPTY.clone()))));
                v
            },
        }
    }
}

impl Inventory for CraftingInventory {
    fn size(&self) -> usize {
        self.items.len()
    }

    fn is_empty(&self) -> bool {
        for slot in self.items.iter() {
            if !slot.lock().is_empty() {
                return false;
            }
        }

        true
    }

    fn get_stack(&self, slot: usize) -> Arc<Mutex<ItemStack>> {
        self.items[slot].clone()
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        let mut removed = ItemStack::EMPTY.clone();
        let mut guard = self.items[slot].lock();
        std::mem::swap(&mut removed, &mut *guard);
        removed
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        split_stack(&self.items, slot, amount)
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        *self.items[slot].lock() = stack;
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl RecipeInputInventory for CraftingInventory {
    fn get_width(&self) -> usize {
        self.width as usize
    }

    fn get_height(&self) -> usize {
        self.height as usize
    }
}

impl Clearable for CraftingInventory {
    fn clear(&self) {
        for slot in self.items.iter() {
            *slot.lock() = ItemStack::EMPTY.clone();
        }
    }
}
