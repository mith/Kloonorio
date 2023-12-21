use bevy::{prelude::*, utils::HashSet};
use tracing::instrument;

use crate::{types::Item, util::try_subtract};

#[derive(Component)]
pub struct Source;

#[derive(Component)]
pub struct Output;

#[derive(Component)]
pub struct Fuel;

#[derive(Component)]
pub struct Storage;

pub const MAX_STACK_SIZE: u32 = 1000;

#[derive(Component, Debug, Clone, PartialEq, Eq, Hash, Reflect)]
pub struct Stack {
    pub item: Item,
    pub amount: u32,
}

impl Stack {
    pub fn new(resource: Item, amount: u32) -> Self {
        Self {
            item: resource,
            amount,
        }
    }

    /// Add an amount to the stack, returning the amount that could not be added.
    pub fn add(&mut self, amount: u32) -> u32 {
        if self.amount + amount > MAX_STACK_SIZE {
            let overflow = self.amount + amount - MAX_STACK_SIZE;
            self.amount = MAX_STACK_SIZE;
            overflow
        } else {
            self.amount += amount;
            0
        }
    }
}

pub type Slot = Option<Stack>;

#[derive(Component, Debug)]
pub struct Inventory {
    pub slots: Vec<Slot>,
    pub allowed_items: ItemFilter,
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub enum ItemFilter {
    All,
    Only(HashSet<Item>),
}

impl Inventory {
    pub fn new(size: u32) -> Self {
        Self {
            slots: vec![None; size as usize],
            allowed_items: ItemFilter::All,
        }
    }

    pub fn new_with_filter(size: u32, allowed_products: HashSet<Item>) -> Self {
        Self {
            slots: vec![None; size as usize],
            allowed_items: ItemFilter::Only(allowed_products),
        }
    }

    /// Return true if the inventory has enough space for the items
    pub fn can_add(&self, items: &[(Item, u32)]) -> bool {
        if let ItemFilter::Only(allowed_products) = &self.allowed_items {
            for (product, _) in items {
                if !allowed_products.contains(product) {
                    return false;
                }
            }
        }

        let mut slots = self.slots.clone();
        let items = items.to_vec();
        for (item_resource, mut item_amount) in items {
            let mut added = false;
            for slot in slots.iter_mut() {
                if let Some(stack) = slot {
                    if stack.item == item_resource {
                        if stack.amount + item_amount <= MAX_STACK_SIZE {
                            stack.amount += item_amount;
                            added = true;
                            break;
                        } else {
                            let diff = MAX_STACK_SIZE - stack.amount;
                            stack.amount = MAX_STACK_SIZE;
                            item_amount -= diff;
                        }
                    }
                } else {
                    *slot = Some(Stack::new(item_resource, item_amount));
                    added = true;
                    break;
                }
            }
            if !added {
                return false;
            }
        }
        true
    }

    /// Add the items to the inventory, returning the remainder
    pub fn add_items(&mut self, items: &[(Item, u32)]) -> Vec<(Item, u32)> {
        let mut remainder = Vec::new();
        for (resource, amount) in items {
            let mut amount = *amount;

            // First iterate over existing stacks
            for stack in self.slots.iter_mut().flatten() {
                if stack.item == *resource {
                    let space = MAX_STACK_SIZE - stack.amount;
                    if space >= amount {
                        stack.amount += amount;
                        amount = 0;
                    } else {
                        stack.amount = MAX_STACK_SIZE;
                        amount -= space;
                    }
                }
                if amount == 0 {
                    break;
                }
            }

            if amount == 0 {
                return remainder;
            }

            // Then put in the first empty slot
            if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
                *slot = Some(Stack {
                    item: resource.clone(),
                    amount: amount.min(MAX_STACK_SIZE),
                });
                amount = 0;
            }

            if amount > 0 {
                remainder.push((resource.clone(), amount));
            }
        }

        remainder
    }

    pub fn add_item(&mut self, item: Item, amount: u32) -> Vec<(Item, u32)> {
        self.add_items(&[(item, amount)])
    }

    pub fn has_item(&self, item: &Item) -> bool {
        self.slots.iter().any(|s| {
            if let Some(stack) = s {
                stack.item == *item
            } else {
                false
            }
        })
    }

    pub fn has_items(&self, items: &[(Item, u32)]) -> bool {
        let mut remaining = items.to_vec();

        for slot in self.slots.iter() {
            if let Some(stack) = slot {
                for (resource, amount) in remaining.iter_mut() {
                    if *resource == stack.item {
                        *amount = amount.saturating_sub(stack.amount);
                    }
                }
            }
            // Check if all items have been found
            if remaining.iter().all(|(_, amount)| *amount == 0) {
                return true;
            }
        }

        // If any item is still required (amount > 0), return false
        !remaining.iter().any(|(_, amount)| *amount > 0)
    }

    pub fn num_items(&self, resource: &Item) -> u32 {
        let mut amount = 0;
        for slot in self.slots.iter() {
            if let Some(stack) = slot {
                if stack.item == *resource {
                    amount += stack.amount;
                }
            }
        }
        amount
    }

    /// Removes all items atomically, returning true on success
    pub fn remove_items(&mut self, items: &[(Item, u32)]) -> bool {
        if !self.has_items(items) {
            return false;
        }

        for (resource, amount) in items {
            let mut amount_to_remove = *amount;
            for slot in self.slots.iter_mut() {
                if let Some(stack) = slot.as_mut() {
                    if stack.item == *resource && amount_to_remove > 0 {
                        let removed_amount = try_subtract(&mut stack.amount, amount_to_remove);
                        amount_to_remove -= removed_amount;

                        if stack.amount == 0 {
                            *slot = None; // Clear the slot if the stack is empty
                        }
                    }
                }
            }
        }
        true
    }

    /// Take a stack of items from the inventory, returning a stack of
    /// items where the amount is min(requested_amount, amount_in_inventory)
    pub fn try_take_item(&mut self, item: &Item, requested_amount: u32) -> Option<Stack> {
        let mut amount = 0;
        for slot in self.slots.iter_mut() {
            if let Some(stack) = slot {
                if stack.item == *item {
                    let taken = try_subtract(&mut stack.amount, requested_amount - amount);
                    amount += taken;

                    if stack.amount == 0 {
                        *slot = None;
                    }
                }
            }
        }

        if amount == 0 {
            return None;
        }

        Some(Stack::new(item.clone(), amount))
    }

    pub fn add_stack(&mut self, stack: Stack) -> Option<Stack> {
        let mut stack = stack;
        for slot in self.slots.iter_mut() {
            if let Some(existing_stack) = slot {
                if existing_stack.item == stack.item {
                    let overflow = existing_stack.add(stack.amount);
                    if overflow == 0 {
                        return None;
                    } else {
                        stack.amount = overflow;
                    }
                }
            } else {
                *slot = Some(stack);
                return None;
            }
        }
        Some(stack)
    }

    pub fn find_item(&self, item: &str) -> Option<usize> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            if let Some(stack) = slot {
                if stack.item.to_string() == item {
                    Some(index)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    pub fn has_space_for(&self, stack: &Stack) -> bool {
        for slot in self.slots.iter() {
            if let Some(existing_stack) = slot {
                if existing_stack.item == stack.item {
                    if existing_stack.amount + stack.amount <= MAX_STACK_SIZE {
                        return true;
                    }
                }
            } else {
                return true;
            }
        }
        false
    }

    pub fn has_space_for_item(&self, item: &Item) -> bool {
        for slot in self.slots.iter() {
            if let Some(existing_stack) = slot {
                if existing_stack.item == *item {
                    if existing_stack.amount < MAX_STACK_SIZE {
                        return true;
                    }
                }
            } else {
                return true;
            }
        }
        false
    }
}

pub fn transfer_between_slots(source_slot: &mut Slot, target_slot: &mut Slot) {
    if let Some(ref mut source_stack) = source_slot {
        if let Some(ref mut target_stack) = target_slot {
            transfer_between_stacks(source_stack, target_stack);
            if source_stack.amount == 0 {
                *source_slot = None;
            }
        } else {
            debug!("Moving source stack to target slot");
            *target_slot = Some(source_stack.clone());
            *source_slot = None;
        }
    }
}

#[instrument(skip(inventory))]
pub fn drop_within_inventory(inventory: &mut Inventory, source_slot: usize, target_slot: usize) {
    if let Some(mut source_stack) = inventory.slots.get(source_slot).cloned().flatten() {
        if let Some(mut target_stack) = inventory.slots.get(target_slot).cloned().flatten() {
            transfer_between_stacks(&mut source_stack, &mut target_stack);
            inventory.slots[target_slot] = Some(target_stack);
            inventory.slots[source_slot] = {
                if source_stack.amount > 0 {
                    debug!(source_stack = ?source_stack, "Keeping source stack");
                    Some(source_stack)
                } else {
                    debug!("Dropping source stack");
                    None
                }
            };
        } else {
            debug!("Moving source stack to target slot");
            inventory.slots[target_slot] = Some(source_stack.clone());
            inventory.slots[source_slot] = None;
        }
    } else {
        error!("Source slot is empty");
    }
}

#[instrument]
pub fn transfer_between_stacks(source_stack: &mut Stack, target_stack: &mut Stack) {
    if source_stack == target_stack {
        return;
    }
    if target_stack.item == source_stack.item {
        debug!("Adding source stack to target stack");
        let remainder = target_stack.add(source_stack.amount);
        source_stack.amount = remainder;
    } else {
        debug!("Swapping stacks");
        std::mem::swap(source_stack, target_stack);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn has_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert!(inventory.has_items(&[(Item::new("Stone"), 5), (Item::new("Wood"), 10)]));
        assert!(!inventory.has_items(&[(Item::new("Stone"), 5), (Item::new("Wood"), 30)]));
    }

    #[test]
    fn has_items_exact() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert!(inventory.has_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]));
    }

    #[test]
    fn remove_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert!(inventory.remove_items(&[(Item::new("Stone"), 5), (Item::new("Wood"), 10)]));
        assert_eq!(inventory.slots[0], Some(Stack::new(Item::new("Stone"), 5)));
        assert_eq!(inventory.slots[1], Some(Stack::new(Item::new("Wood"), 10)));
    }

    #[test]
    fn remove_items_empty() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert!(inventory.remove_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]));
        assert!(inventory.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn remove_items_not_enough() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert!(!inventory.remove_items(&[(Item::new("Stone"), 5), (Item::new("Wood"), 30)]));
        assert_eq!(inventory.slots[0], Some(Stack::new(Item::new("Stone"), 10)));
        assert_eq!(inventory.slots[1], Some(Stack::new(Item::new("Wood"), 20)));
    }

    #[test]
    fn remove_items_not_in_inventory() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 5)]);
        assert!(!inventory.remove_items(&[(Item::new("Wood"), 1)]));
        assert_eq!(inventory.slots[0], Some(Stack::new(Item::new("Stone"), 5)));
    }

    #[test]
    fn add_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert_eq!(inventory.slots[0], Some(Stack::new(Item::new("Stone"), 10)));
        assert_eq!(inventory.slots[1], Some(Stack::new(Item::new("Wood"), 20)));
    }

    #[test]
    fn add_items_remainder() {
        let mut inventory = Inventory::new(1);
        let remainder = inventory.add_items(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]);
        assert_eq!(inventory.slots[0], Some(Stack::new(Item::new("Stone"), 10)));
        assert_eq!(remainder, vec![(Item::new("Wood"), 20)]);
    }

    #[test]
    fn add_items_stack() {
        let mut inventory = Inventory::new(2);
        inventory.slots[1] = Some(Stack::new(Item::new("Stone furnace"), 10));
        inventory.add_items(&[(Item::new("Stone furnace"), 1)]);
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Item::new("Stone furnace"), 11))
        );
    }

    #[test]
    fn add_stack() {
        let mut inventory = Inventory::new(12);
        inventory.add_stack(Stack::new(Item::new("Stone"), 10));
        assert_eq!(inventory.slots[0], Some(Stack::new(Item::new("Stone"), 10)));
    }

    #[test]
    fn add_stack_remainder() {
        let mut inventory = Inventory::new(0);
        let remainder = inventory.add_stack(Stack::new(Item::new("Stone"), 10));
        assert_eq!(remainder, Some(Stack::new(Item::new("Stone"), 10)));
    }

    #[test]
    fn try_take_item() {
        let mut inventory = Inventory::new(12);
        inventory.add_stack(Stack::new(Item::new("Stone"), 10));
        let taken = inventory.try_take_item(&Item::new("Stone"), 100);

        assert_eq!(taken, Some(Stack::new(Item::new("Stone"), 10)));
        assert!(inventory.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn try_take_item_clear_empty_slot() {
        let mut inventory = Inventory::new(12);
        inventory.slots[0] = Some(Stack::new(Item::new("Stone"), 10));
        let taken = inventory.try_take_item(&Item::new("Stone"), 100);

        assert_eq!(taken, Some(Stack::new(Item::new("Stone"), 10)));
        assert!(inventory.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn transfer_between_stacks_swap() {
        let mut source_stack = Stack::new(Item::new("Stone"), 10);
        let mut target_stack = Stack::new(Item::new("Iron ore"), 20);

        transfer_between_stacks(&mut source_stack, &mut target_stack);

        assert_eq!(source_stack, Stack::new(Item::new("Iron ore"), 20));
        assert_eq!(target_stack, Stack::new(Item::new("Stone"), 10));
    }

    #[test]
    fn transfer_between_stacks_same() {
        let mut source_stack = Stack::new(Item::new("Stone"), 10);
        let mut target_stack = Stack::new(Item::new("Stone"), 20);

        transfer_between_stacks(&mut source_stack, &mut target_stack);

        assert_eq!(source_stack, Stack::new(Item::new("Stone"), 0));
        assert_eq!(target_stack, Stack::new(Item::new("Stone"), 30));
    }

    #[test]
    fn transfer_between_slots_swap() {
        let mut source_slot = Some(Stack::new(Item::new("Stone"), 10));
        let mut target_slot = Some(Stack::new(Item::new("Iron ore"), 20));

        transfer_between_slots(&mut source_slot, &mut target_slot);

        assert_eq!(source_slot, Some(Stack::new(Item::new("Iron ore"), 20)));
        assert_eq!(target_slot, Some(Stack::new(Item::new("Stone"), 10)));
    }

    #[test]
    fn transfer_between_slots_merge_stacks() {
        let mut source_slot = Some(Stack::new(Item::new("Stone"), 10));
        let mut target_slot = Some(Stack::new(Item::new("Stone"), 20));

        transfer_between_slots(&mut source_slot, &mut target_slot);

        assert_eq!(source_slot, None);
        assert_eq!(target_slot, Some(Stack::new(Item::new("Stone"), 30)));
    }

    #[test]
    fn transfer_between_slots_empty() {
        let mut source_slot = Some(Stack::new(Item::new("Stone"), 10));
        let mut target_slot = None;

        transfer_between_slots(&mut source_slot, &mut target_slot);

        assert_eq!(source_slot, None);
        assert_eq!(target_slot, Some(Stack::new(Item::new("Stone"), 10)));
    }

    #[test]
    fn drop_within_inventory_swap() {
        let mut inventory = Inventory::new(12);

        inventory.slots[0] = Some(Stack::new(Item::new("Stone"), 10));
        inventory.slots[1] = Some(Stack::new(Item::new("Iron ore"), 20));

        drop_within_inventory(&mut inventory, 1, 0);

        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Item::new("Iron ore"), 20))
        );
        assert_eq!(inventory.slots[1], Some(Stack::new(Item::new("Stone"), 10)));
    }
}
