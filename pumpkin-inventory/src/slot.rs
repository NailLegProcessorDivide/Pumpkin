#![warn(unused)]
use std::{
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use crate::screen_handler::InventoryPlayer;

use parking_lot::Mutex;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::item::Item;
use pumpkin_world::inventory::Inventory;
use pumpkin_world::item::ItemStack;

// Slot.java
// This is a trait due to crafting slots being a thing

pub trait Slot: Send + Sync + Debug {
    fn get_inventory(&self) -> Arc<dyn Inventory>;

    fn get_index(&self) -> usize;

    fn set_id(&self, index: usize);

    /// Used to notify result slots that they need to update their contents. (e.g. refill)
    /// Note that you **MUST** call this after changing the stack in the slot, and releasing any
    /// locks to the stack to avoid deadlocks.
    ///
    /// Also see: `ScreenHandler::quick_move`
    fn on_quick_move_crafted(&self, _stack: ItemStack, _stack_prev: ItemStack) {}

    /// Callback for when an item is taken from the slot.
    ///
    /// Also see: `safe_take`
    fn on_take_item(&self, _player: &dyn InventoryPlayer, _stack: &ItemStack) {
        self.mark_dirty();
    }

    // Used for plugins
    fn on_click(&self, _player: &dyn InventoryPlayer) {}

    fn can_insert(&self, _stack: &ItemStack) -> bool {
        true
    }

    fn get_stack(&self) -> Arc<Mutex<ItemStack>> {
        self.get_inventory().get_stack(self.get_index())
    }

    fn get_cloned_stack(&self) -> ItemStack {
        let stack = self.get_stack();
        let lock = stack
            .try_lock_for(Duration::from_secs(5))
            .expect("Timed out while trying to acquire lock");

        lock.clone()
    }

    fn has_stack(&self) -> bool {
        let inv = self.get_inventory();
        !inv.get_stack(self.get_index()).lock().is_empty()
    }

    /// Make sure to drop any locks to the slot stack before calling this
    fn set_stack(&self, stack: ItemStack) {
        self.set_stack_no_callbacks(stack);
    }

    /// Changes the stack in the slot with the given `stack`.
    fn set_stack_prev(&self, stack: ItemStack, _previous_stack: ItemStack) {
        self.set_stack_no_callbacks(stack);
    }

    fn set_stack_no_callbacks(&self, stack: ItemStack) {
        let inv = self.get_inventory();
        inv.set_stack(self.get_index(), stack);
        self.mark_dirty();
    }

    fn mark_dirty(&self);

    fn get_max_item_count(&self) -> u8 {
        self.get_inventory().get_max_count_per_stack()
    }

    fn get_max_item_count_for_stack(&self, stack: &ItemStack) -> u8 {
        self.get_max_item_count().min(stack.get_max_stack_size())
    }

    /// Removes a specific amount of items from the slot.
    ///
    /// Mojang name: `remove`
    fn take_stack(&self, amount: u8) -> ItemStack {
        let inv = self.get_inventory();

        inv.remove_stack_specific(self.get_index(), amount)
    }

    /// Mojang name: `mayPickup`
    fn can_take_items(&self, _player: &dyn InventoryPlayer) -> bool {
        true
    }

    /// Mojang name: `allowModification`
    fn allow_modification(&self, player: &dyn InventoryPlayer) -> bool {
        self.can_insert(&self.get_cloned_stack()) && self.can_take_items(player)
    }

    /// Mojang name: `tryRemove`
    fn try_take_stack_range(
        &self,
        min: u8,
        max: u8,
        player: &dyn InventoryPlayer,
    ) -> Option<ItemStack> {
        if !self.can_take_items(player) {
            return None;
        }
        if !self.allow_modification(player) && self.get_cloned_stack().item_count > max {
            // If the slot is not allowed to be modified, we cannot take a partial stack from it.
            return None;
        }
        let min = min.min(max);
        let stack = self.take_stack(min);

        if stack.is_empty() {
            None
        } else {
            if self.get_cloned_stack().is_empty() {
                self.set_stack_prev(ItemStack::EMPTY.clone(), stack.clone());
            }

            Some(stack)
        }
    }

    /// Safely tries to take a stack of items from the slot, returning `None` if the stack is empty.
    /// Considering such as result slots, as their stacks cannot split.
    ///
    /// Mojang name: `safeTake`
    fn safe_take(&self, min: u8, max: u8, player: &dyn InventoryPlayer) -> ItemStack {
        let stack = self.try_take_stack_range(min, max, player);

        if let Some(stack) = &stack {
            self.on_take_item(player, stack);
        }

        stack.unwrap_or(ItemStack::EMPTY.clone())
    }

    fn insert_stack(&self, stack: ItemStack) -> ItemStack {
        let stack_item_count = stack.item_count;
        self.insert_stack_count(stack, stack_item_count)
    }

    fn insert_stack_count(&self, mut stack: ItemStack, count: u8) -> ItemStack {
        if !stack.is_empty() && self.can_insert(&stack) {
            let stack_mutex = self.get_stack();
            let mut stack_self = stack_mutex.lock();
            let min_count = count
                .min(stack.item_count)
                .min(self.get_max_item_count_for_stack(&stack) - stack_self.item_count);

            if min_count == 0 {
                return stack;
            } else {
                if stack_self.is_empty() {
                    drop(stack_self);
                    self.set_stack(stack.split(min_count));
                } else if stack.are_items_and_components_equal(&stack_self) {
                    stack.decrement(min_count);
                    stack_self.increment(min_count);
                    let cloned_stack = stack_self.clone();
                    drop(stack_self);
                    self.set_stack(cloned_stack);
                }

                return stack;
            }
        } else {
            stack
        }
    }
}

#[derive(Debug)]
/// Just called Slot in Vanilla
pub struct NormalSlot {
    pub inventory: Arc<dyn Inventory>,
    pub index: usize,
    pub id: AtomicU8,
}

impl NormalSlot {
    pub fn new(inventory: Arc<dyn Inventory>, index: usize) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
        }
    }
}

impl Slot for NormalSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}

// ArmorSlot.java
#[derive(Debug)]
pub struct ArmorSlot {
    pub inventory: Arc<dyn Inventory>,
    pub index: usize,
    pub id: AtomicU8,
    pub equipment_slot: EquipmentSlot,
}

impl ArmorSlot {
    pub fn new(inventory: Arc<dyn Inventory>, index: usize, equipment_slot: EquipmentSlot) -> Self {
        Self {
            inventory,
            index,
            id: AtomicU8::new(0),
            equipment_slot,
        }
    }
}

impl Slot for ArmorSlot {
    fn get_inventory(&self) -> Arc<dyn Inventory> {
        self.inventory.clone()
    }

    fn get_index(&self) -> usize {
        self.index
    }

    fn set_id(&self, id: usize) {
        self.id.store(id as u8, Ordering::Relaxed);
    }

    fn get_max_item_count(&self) -> u8 {
        1
    }

    fn set_stack_prev(&self, stack: ItemStack, _previous_stack: ItemStack) {
        //TODO: this.entity.onEquipStack(this.equipmentSlot, previousStack, stack);
        self.set_stack_no_callbacks(stack);
    }

    fn can_insert(&self, stack: &ItemStack) -> bool {
        match self.equipment_slot {
            EquipmentSlot::Head(_) => {
                stack.is_helmet() || stack.is_skull() || stack.item == &Item::CARVED_PUMPKIN
            }
            EquipmentSlot::Chest(_) => stack.is_chestplate() || stack.item == &Item::ELYTRA,
            EquipmentSlot::Legs(_) => stack.is_leggings(),
            EquipmentSlot::Feet(_) => stack.is_boots(),
            _ => true,
        }
    }

    fn can_take_items(&self, _player: &dyn InventoryPlayer) -> bool {
        // TODO: Check enchantments
        true
    }

    fn mark_dirty(&self) {
        self.inventory.mark_dirty();
    }
}
