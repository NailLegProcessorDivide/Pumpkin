use std::{any::Any, sync::Arc};

use parking_lot::Mutex;
use pumpkin_world::{
    inventory::{Clearable, Inventory},
    item::ItemStack,
};

#[derive(Debug)]
pub struct DoubleInventory {
    first: Arc<dyn Inventory>,
    second: Arc<dyn Inventory>,
}

impl DoubleInventory {
    pub fn new(first: Arc<dyn Inventory>, second: Arc<dyn Inventory>) -> Arc<Self> {
        Arc::new(Self { first, second })
    }
}

impl Inventory for DoubleInventory {
    fn size(&self) -> usize {
        self.first.size() + self.second.size()
    }

    fn is_empty(&self) -> bool {
        self.first.is_empty() && self.second.is_empty()
    }

    fn get_stack(&self, slot: usize) -> Arc<Mutex<ItemStack>> {
        if slot >= self.first.size() {
            self.second.get_stack(slot - self.first.size())
        } else {
            self.first.get_stack(slot)
        }
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot >= self.first.size() {
            self.second.remove_stack(slot - self.first.size())
        } else {
            self.first.remove_stack(slot)
        }
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot >= self.first.size() {
            self.second
                .remove_stack_specific(slot - self.first.size(), amount)
        } else {
            self.first.remove_stack_specific(slot, amount)
        }
    }

    fn get_max_count_per_stack(&self) -> u8 {
        self.first.get_max_count_per_stack()
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot >= self.first.size() {
            self.second.set_stack(slot - self.first.size(), stack)
        } else {
            self.first.set_stack(slot, stack)
        }
    }

    fn mark_dirty(&self) {
        self.first.mark_dirty();
        self.second.mark_dirty();
    }

    fn on_open(&self) {
        self.first.on_open();
        self.second.on_open();
    }

    fn on_close(&self) {
        self.first.on_close();
        self.second.on_close();
    }

    fn is_valid_slot_for(&self, slot: usize, stack: &ItemStack) -> bool {
        if slot >= self.first.size() {
            self.second
                .is_valid_slot_for(slot - self.first.size(), stack)
        } else {
            self.first.is_valid_slot_for(slot, stack)
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clearable for DoubleInventory {
    fn clear(&self) {
        self.first.clear();
        self.second.clear();
    }
}
