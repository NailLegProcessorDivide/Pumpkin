use crate::entity_equipment::EntityEquipment;
use crate::screen_handler::InventoryPlayer;

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_protocol::java::client::play::CSetPlayerInventory;
use pumpkin_util::Hand;
use pumpkin_world::inventory::split_stack;
use pumpkin_world::inventory::{Clearable, Inventory};
use pumpkin_world::item::ItemStack;
use std::any::Any;
use std::array::from_fn;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use parking_lot::Mutex;

#[derive(Debug)]
pub struct PlayerInventory {
    pub main_inventory: [Arc<Mutex<ItemStack>>; Self::MAIN_SIZE],
    pub equipment_slots: Arc<HashMap<usize, EquipmentSlot>>,
    selected_slot: AtomicU8,
    pub entity_equipment: Arc<Mutex<EntityEquipment>>,
}

impl PlayerInventory {
    pub const MAIN_SIZE: usize = 36;
    const HOTBAR_SIZE: usize = 9;
    pub const OFF_HAND_SLOT: usize = 40;

    // TODO: Add inventory load from nbt
    pub fn new(
        entity_equipment: Arc<Mutex<EntityEquipment>>,
        equipment_slots: Arc<HashMap<usize, EquipmentSlot>>,
    ) -> Self {
        Self {
            // Normal syntax can't be used here because Arc doesn't implement Copy
            main_inventory: from_fn(|_| Arc::new(Mutex::new(ItemStack::EMPTY.clone()))),
            equipment_slots,
            selected_slot: AtomicU8::new(0),
            entity_equipment,
        }
    }

    /// getSelectedStack in source
    pub fn held_item(&self) -> Arc<Mutex<ItemStack>> {
        self.main_inventory
            .get(self.get_selected_slot() as usize)
            .unwrap()
            .clone()
    }

    pub fn get_stack_in_hand(&self, hand: Hand) -> Arc<Mutex<ItemStack>> {
        match hand {
            Hand::Left => self.off_hand_item(),
            Hand::Right => self.held_item(),
        }
    }

    /// getOffHandStack in source
    pub fn off_hand_item(&self) -> Arc<Mutex<ItemStack>> {
        let slot = self
            .equipment_slots
            .get(&PlayerInventory::OFF_HAND_SLOT)
            .unwrap();
        self.entity_equipment.lock().get(slot)
    }

    pub fn swap_item(&self) -> (ItemStack, ItemStack) {
        let slot = self
            .equipment_slots
            .get(&PlayerInventory::OFF_HAND_SLOT)
            .unwrap();
        let mut equipment = self.entity_equipment.lock();
        let binding = self.held_item();
        let mut main_hand_item = binding.lock();
        let off_hand_item = main_hand_item.clone();
        *main_hand_item = equipment.put(slot, off_hand_item.clone());
        (main_hand_item.clone(), off_hand_item)
    }

    pub fn is_valid_hotbar_index(slot: usize) -> bool {
        slot < Self::HOTBAR_SIZE
    }

    fn add_stack(&self, stack: ItemStack) -> usize {
        let mut slot_index = self.get_occupied_slot_with_room_for_stack(&stack);

        if slot_index == -1 {
            slot_index = self.get_empty_slot();
        }

        if slot_index == -1 {
            stack.item_count as usize
        } else {
            return self.add_stack_to_slot(slot_index as usize, stack);
        }
    }

    fn add_stack_to_slot(&self, slot: usize, stack: ItemStack) -> usize {
        let mut stack_count = stack.item_count;
        let binding = self.get_stack(slot);
        let mut self_stack = binding.lock();

        if self_stack.is_empty() {
            *self_stack = stack.copy_with_count(0);
            //self.set_stack(slot, self_stack);
        }

        let count_left = self_stack.get_max_stack_size() - self_stack.item_count;
        let count_min = stack_count.min(count_left);

        if count_min == 0 {
            stack_count as usize
        } else {
            stack_count -= count_min;
            self_stack.increment(count_min);
            stack_count as usize
        }
    }

    fn get_empty_slot(&self) -> i16 {
        for i in 0..Self::MAIN_SIZE {
            if self.main_inventory[i].lock().is_empty() {
                return i as i16;
            }
        }

        -1
    }

    fn can_stack_add_more(&self, existing_stack: &ItemStack, stack: &ItemStack) -> bool {
        !existing_stack.is_empty()
            && existing_stack.are_items_and_components_equal(stack)
            && existing_stack.is_stackable()
            && existing_stack.item_count < existing_stack.get_max_stack_size()
    }

    fn get_occupied_slot_with_room_for_stack(&self, stack: &ItemStack) -> i16 {
        if self.can_stack_add_more(
            &*self.get_stack(self.get_selected_slot() as usize).lock(),
            stack,
        ) {
            self.get_selected_slot() as i16
        } else if self.can_stack_add_more(&*self.get_stack(Self::OFF_HAND_SLOT).lock(), stack) {
            Self::OFF_HAND_SLOT as i16
        } else {
            for i in 0..Self::MAIN_SIZE {
                if self.can_stack_add_more(&*self.main_inventory[i].lock(), stack) {
                    return i as i16;
                }
            }

            -1
        }
    }

    pub fn insert_stack_anywhere(&self, stack: &mut ItemStack) -> bool {
        self.insert_stack(-1, stack)
    }

    pub fn insert_stack(&self, slot: i16, stack: &mut ItemStack) -> bool {
        if stack.is_empty() {
            return false;
        }

        // TODO: if (stack.isDamaged()) {

        let mut i;

        loop {
            i = stack.item_count;
            if slot == -1 {
                stack.set_count(self.add_stack(stack.clone()) as u8);
            } else {
                stack.set_count(self.add_stack_to_slot(slot as usize, stack.clone()) as u8);
            }

            if stack.is_empty() || stack.item_count >= i {
                break;
            }
        }

        // TODO: Creative mode check

        stack.item_count < i
    }

    pub fn get_slot_with_stack(&self, stack: &ItemStack) -> i16 {
        for i in 0..Self::MAIN_SIZE {
            if !self.main_inventory[i].lock().is_empty()
                && self.main_inventory[i]
                    .lock()
                    .are_items_and_components_equal(stack)
            {
                return i as i16;
            }
        }

        -1
    }

    pub fn get_swappable_hotbar_slot(&self) -> usize {
        let selected_slot = self.get_selected_slot() as usize;
        for i in 0..Self::HOTBAR_SIZE {
            let check_index = (i + selected_slot) % 9;
            if self.main_inventory[check_index].lock().is_empty() {
                return check_index;
            }
        }

        for i in 0..Self::HOTBAR_SIZE {
            let check_index = (i + selected_slot) % 9;
            if true
            /*TODO: If item has an enchantment skip it */
            {
                return check_index;
            }
        }

        self.get_selected_slot() as usize
    }

    pub fn swap_stack_with_hotbar(&self, stack: ItemStack) {
        self.set_selected_slot(self.get_swappable_hotbar_slot() as u8);

        if !self.main_inventory[self.get_selected_slot() as usize]
            .lock()
            .is_empty()
        {
            let empty_slot = self.get_empty_slot();
            if empty_slot != -1 {
                self.set_stack(
                    empty_slot as usize,
                    self.main_inventory[self.get_selected_slot() as usize]
                        .lock()
                        .clone(),
                );
            }
        }

        self.set_stack(self.get_selected_slot() as usize, stack);
    }

    pub fn swap_slot_with_hotbar(&self, slot: usize) {
        self.set_selected_slot(self.get_swappable_hotbar_slot() as u8);
        let stack = self.main_inventory[self.get_selected_slot() as usize]
            .lock()
            .clone();
        self.set_stack(
            self.get_selected_slot() as usize,
            self.main_inventory[slot].lock().clone(),
        );
        self.set_stack(slot, stack);
    }

    pub fn offer_or_drop_stack(&self, stack: ItemStack, player: &dyn InventoryPlayer) {
        self.offer(stack, true, player);
    }

    pub fn offer(&self, stack: ItemStack, notify_client: bool, player: &dyn InventoryPlayer) {
        let mut stack = stack;
        while !stack.is_empty() {
            let mut room_for_stack = self.get_occupied_slot_with_room_for_stack(&stack);
            if room_for_stack == -1 {
                room_for_stack = self.get_empty_slot();
            }

            if room_for_stack == -1 {
                player.drop_item(stack, false);
                break;
            }

            let items_fit = stack.get_max_stack_size()
                - self.get_stack(room_for_stack as usize).lock().item_count;
            if self.insert_stack(room_for_stack, &mut stack.split(items_fit)) && notify_client {
                player.enqueue_slot_set_packet(&CSetPlayerInventory::new(
                    (room_for_stack as i32).into(),
                    &stack.clone().into(),
                ));
            }
        }
    }
}

impl Clearable for PlayerInventory {
    fn clear(&self) {
        for item in self.main_inventory.iter() {
            *item.lock() = ItemStack::EMPTY.clone();
        }

        self.entity_equipment.lock().clear();
    }
}

impl Inventory for PlayerInventory {
    fn size(&self) -> usize {
        self.main_inventory.len() + self.equipment_slots.len()
    }

    fn is_empty(&self) -> bool {
        for item in self.main_inventory.iter() {
            if !item.lock().is_empty() {
                return false;
            }
        }

        for slot in self.equipment_slots.values() {
            if !self.entity_equipment.lock().get(slot).lock().is_empty() {
                return false;
            }
        }

        true
    }

    fn get_stack(&self, slot: usize) -> Arc<Mutex<ItemStack>> {
        if slot < self.main_inventory.len() {
            self.main_inventory[slot].clone()
        } else {
            let slot = self.equipment_slots.get(&slot).unwrap();
            self.entity_equipment.lock().get(slot)
        }
    }

    fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
        if slot < self.main_inventory.len() {
            split_stack(&self.main_inventory, slot, amount)
        } else {
            let slot = self.equipment_slots.get(&slot).unwrap();

            let equipment = self.entity_equipment.lock().get(slot);
            let mut stack = equipment.lock();

            if !stack.is_empty() {
                return stack.split(amount);
            }

            ItemStack::EMPTY.clone()
        }
    }

    fn remove_stack(&self, slot: usize) -> ItemStack {
        if slot < self.main_inventory.len() {
            let mut removed = ItemStack::EMPTY.clone();
            let mut guard = self.main_inventory[slot].lock();
            std::mem::swap(&mut removed, &mut *guard);
            removed
        } else {
            let slot = self.equipment_slots.get(&slot).unwrap();
            self.entity_equipment
                .lock()
                .put(slot, ItemStack::EMPTY.clone())
        }
    }

    fn set_stack(&self, slot: usize, stack: ItemStack) {
        if slot < self.main_inventory.len() {
            *self.main_inventory[slot].lock() = stack;
        } else {
            match self.equipment_slots.get(&slot) {
                Some(slot) => {
                    self.entity_equipment.lock().put(slot, stack);
                }
                None => log::warn!("Failed to get Equipment Slot at {slot}"),
            }
        }
    }

    fn mark_dirty(&self) {}

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl PlayerInventory {
    pub fn set_selected_slot(&self, slot: u8) {
        if Self::is_valid_hotbar_index(slot as usize) {
            self.selected_slot.store(slot, Ordering::Relaxed);
        } else {
            panic!("Invalid hotbar slot: {slot}");
        }
    }

    pub fn get_selected_slot(&self) -> u8 {
        self.selected_slot.load(Ordering::Relaxed)
    }
}
