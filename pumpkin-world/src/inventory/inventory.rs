use crate::item::ItemStack;

use parking_lot::Mutex;
use pumpkin_data::item::Item;
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use std::any::Any;
use std::{
    fmt::Debug,
    hash::{Hash, Hasher},
    sync::Arc,
};

// Inventory.java

pub trait Inventory: Send + Sync + Debug + Clearable {
    fn size(&self) -> usize;

    fn is_empty(&self) -> bool;

    fn get_stack(&self, slot: usize) -> Arc<Mutex<ItemStack>>;

    fn remove_stack(&self, slot: usize) -> ItemStack;

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack;

    fn get_max_count_per_stack(&self) -> u8 {
        99
    }

    fn set_stack(&self, slot: usize, stack: ItemStack);

    fn mark_dirty(&self) {}

    fn write_data(
        &self,
        nbt: &mut pumpkin_nbt::compound::NbtCompound,
        stacks: &[Arc<Mutex<ItemStack>>],
        include_empty: bool,
    ) {
        let mut slots = Vec::new();

        for (i, item) in stacks.iter().enumerate() {
            let stack = item.lock();
            if !stack.is_empty() {
                let mut item_compound = NbtCompound::new();
                item_compound.put_byte("Slot", i as i8);
                stack.write_item_stack(&mut item_compound);
                slots.push(NbtTag::Compound(item_compound));
            }
        }

        if !include_empty && slots.is_empty() {
            return;
        }

        nbt.put("Items", NbtTag::List(slots));
    }

    fn read_data(
        &self,
        nbt: &pumpkin_nbt::compound::NbtCompound,
        stacks: &[Arc<Mutex<ItemStack>>],
    ) {
        if let Some(inventory_list) = nbt.get_list("Items") {
            for tag in inventory_list {
                if let Some(item_compound) = tag.extract_compound()
                    && let Some(slot_byte) = item_compound.get_byte("Slot")
                {
                    let slot = slot_byte as usize;
                    if slot < stacks.len()
                        && let Some(item_stack) = ItemStack::read_item_stack(item_compound)
                    {
                        // This won't error cause it's only called on initialization
                        *stacks[slot].try_lock().unwrap() = item_stack;
                    }
                }
            }
        }
    }

    /*
    boolean canPlayerUse(PlayerEntity player);
    */

    // TODO: Add (PlayerEntity player)
    fn on_open(&self) {}
    fn on_close(&self) {}

    /// isValid is source
    fn is_valid_slot_for(&self, _slot: usize, _stack: &ItemStack) -> bool {
        true
    }

    fn can_transfer_to(
        &self,
        _hopper_inventory: &dyn Inventory,
        _slot: usize,
        _stack: &ItemStack,
    ) -> bool {
        true
    }

    fn count(&self, item: &Item) -> u8 {
        let mut count = 0;

        for i in 0..self.size() {
            let slot = self.get_stack(i);
            let stack = slot.lock();
            if stack.get_item().id == item.id {
                count += stack.item_count;
            }
        }

        count
    }

    fn contains_any_predicate(&self, predicate: &(dyn Fn(&ItemStack) -> bool)) -> bool {
        for i in 0..self.size() {
            let slot = self.get_stack(i);
            let stack = slot.lock();
            if predicate(&*stack) {
                return true;
            }
        }

        false
    }

    fn contains_any(&self, items: &[Item]) -> bool {
        self.contains_any_predicate(&|stack| !stack.is_empty() && items.contains(stack.get_item()))
    }

    // TODO: canPlayerUse

    fn as_any(&self) -> &dyn Any;
}

pub trait Clearable {
    fn clear(&self);
}

pub struct ComparableInventory(pub Arc<dyn Inventory>);

impl PartialEq for ComparableInventory {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ComparableInventory {}

impl Hash for ComparableInventory {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let ptr = Arc::as_ptr(&self.0);
        ptr.hash(state);
    }
}
