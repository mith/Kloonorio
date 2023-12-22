use bevy::{
    prelude::*,
    utils::{HashMap, HashSet},
};
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

#[derive(Component, Debug, Clone)]
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

    /// Return true if the inventory has enough space for the items.
    pub fn can_add(&self, items: &[(Item, u32)]) -> bool {
        if let ItemFilter::Only(allowed_products) = &self.allowed_items {
            for (product, _) in items {
                if !allowed_products.contains(product) {
                    return false;
                }
            }
        }

        let mut space_needed = HashMap::new();

        // Calculate the space needed for each item.
        for (item_resource, item_amount) in items {
            *space_needed.entry(item_resource).or_insert(0) += item_amount;
        }

        // Check if there's enough space in the existing stacks.
        for stack in self.slots.iter().flatten() {
            if let Some(needed) = space_needed.get_mut(&stack.item) {
                let space_in_slot = MAX_STACK_SIZE - stack.amount;
                if *needed > space_in_slot {
                    *needed -= space_in_slot;
                } else {
                    *needed = 0;
                }
            }
        }

        // Early return if there's no more space needed
        if space_needed.values().all(|&needed| needed == 0) {
            return true;
        }

        // Check if there's enough empty slots for the remaining items.
        let empty_slots = self.slots.iter().filter(|s| s.is_none()).count() as u32;
        let total_slots_needed = space_needed
            .values()
            .map(|&needed| {
                if needed % MAX_STACK_SIZE == 0 {
                    needed / MAX_STACK_SIZE
                } else {
                    (needed / MAX_STACK_SIZE) + 1
                }
            })
            .sum::<u32>();

        total_slots_needed <= empty_slots
    }

    pub fn can_add_item(&self, item: &Item) -> bool {
        self.can_add(&[(item.clone(), 1)])
    }

    /// Add the items to the inventory, returning the remainder
    pub fn add_items(&mut self, items: &[(Item, u32)]) -> Vec<(Item, u32)> {
        let mut remainder = Vec::new();
        for (item, amount) in items {
            let overflow = self.add_item(item, *amount);
            if overflow > 0 {
                remainder.push((item.clone(), overflow));
            }
        }
        remainder
    }

    pub fn add_item(&mut self, item: &Item, amount: u32) -> u32 {
        let mut amount = amount;
        for stack in self.slots.iter_mut().flatten() {
            if stack.item == *item {
                let space_available = MAX_STACK_SIZE - stack.amount;
                if space_available > 0 {
                    let transfer_amount = std::cmp::min(space_available, amount);
                    stack.amount += transfer_amount;
                    amount -= transfer_amount;
                }
            }
        }

        // Check if there is any remaining amount to add as a new stack
        while amount > 0 {
            if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
                let stack_amount = std::cmp::min(amount, MAX_STACK_SIZE);
                *slot = Some(Stack::new(item.clone(), stack_amount));
                amount -= stack_amount;
            } else {
                return amount; // If there are no empty slots, return the remaining amount
            }
        }

        amount
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
        for stack in self.slots.iter().flatten() {
            if stack.item == *resource {
                amount += stack.amount;
            }
        }
        amount
    }

    /// Removes item, returning true on success. Does not remove the item if
    /// the amount to remove is higher than the amount in the inventory.
    fn remove_item(&mut self, item: &Item, amount_to_remove: u32) -> bool {
        if amount_to_remove == 0 {
            return true;
        }

        if self.num_items(item) < amount_to_remove {
            return false;
        }

        let mut amount_to_remove = amount_to_remove;

        // First pass: Target non-full stacks
        if let Some(slot) = self.slots.iter_mut().find(|s| {
            s.as_ref()
                .map(|stack| stack.item == *item && stack.amount < MAX_STACK_SIZE)
                .unwrap_or(false)
        }) {
            if let Some(stack) = slot {
                let removed_amount = try_subtract(&mut stack.amount, amount_to_remove);
                amount_to_remove -= removed_amount;

                if stack.amount == 0 {
                    *slot = None; // Clear the slot if the stack is empty
                }

                if amount_to_remove == 0 {
                    return true;
                }
            }
        }

        // Second pass: Target full stacks if more items need to be removed
        for slot in self.slots.iter_mut() {
            if let Some(stack) = slot {
                if stack.item == *item && stack.amount == MAX_STACK_SIZE {
                    let removed_amount = try_subtract(&mut stack.amount, amount_to_remove);
                    amount_to_remove -= removed_amount;

                    if stack.amount == 0 {
                        *slot = None; // Clear the slot if the stack is empty
                    }

                    if amount_to_remove == 0 {
                        return true;
                    }
                }
            }
        }

        unreachable!("Should have removed all items")
    }

    /// Removes all items atomically, returning true on success
    pub fn remove_items(&mut self, items: &[(Item, u32)]) -> bool {
        if !self.has_items(items) {
            return false;
        }

        items
            .iter()
            .all(|(item, amount)| self.remove_item(item, *amount))
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
        let remainder = self.add_item(&stack.item, stack.amount);
        if remainder > 0 {
            Some(Stack::new(stack.item, remainder))
        } else {
            None
        }
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

    use ahash::{HashMap, HashMapExt};
    use proptest::prelude::*;

    #[derive(Debug, Clone, PartialEq)]
    enum SwapAction {
        MoveToB,
        MoveToA,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum InventoryAction {
        Insert(Item, u32),
        Take(Item, u32),
    }

    fn simulate_inventory_actions(actions: &[InventoryAction]) -> (Inventory, HashMap<Item, u32>) {
        let mut inventory = Inventory::new(10);
        let mut expected_state = HashMap::new();

        for action in actions {
            match action {
                InventoryAction::Insert(item, amount) => {
                    inventory.add_item(&item.clone(), *amount);
                    *expected_state.entry(item.clone()).or_insert(0) += amount;
                }
                InventoryAction::Take(item, amount) => {
                    if let Some(stack) = inventory.try_take_item(item, *amount) {
                        *expected_state.entry(item.clone()).or_insert(0) = expected_state
                            .get(item)
                            .unwrap_or(&0)
                            .saturating_sub(stack.amount);
                    }
                }
            }
        }

        (inventory, expected_state)
    }

    fn count_items_by_type(inventory: &Inventory) -> HashMap<Item, u32> {
        let mut counts = HashMap::new();
        for stack in inventory.slots.iter().flatten() {
            *counts.entry(stack.item.clone()).or_insert(0) += stack.amount;
        }
        counts
    }

    prop_compose! {
        fn arb_product()(product in any::<u32>()) -> Item {
            match product % 4 {
                0 => Item::new("Wood"),
                1 => Item::new("Stone"),
                2 => Item::new("Iron ore"),
                3 => Item::new("Coal"),
                _ => unreachable!(),
            }
        }
    }

    prop_compose! {
        fn arb_inventory(size: u32)(products in prop::collection::vec(arb_product(), 1..(size as usize))) -> Inventory {
            let mut inventory = Inventory::new(size);
            for product in products {
                inventory.add_item(&product, 1);
            }
            inventory
        }
    }

    prop_compose! {
        fn arb_action()(action in any::<u32>(), item in arb_product()) -> (SwapAction, Item) {
            let action = if action % 2 == 0 { SwapAction::MoveToB } else { SwapAction::MoveToA };
            (action, item)
        }
    }

    prop_compose! {
        fn arb_inventory_action()(action in any::<u32>(), item in arb_product(), amount in 1u32..10u32) -> InventoryAction {
            if action % 2 == 0 {
                InventoryAction::Insert(item, amount)
            } else {
                InventoryAction::Take(item, amount)
            }
        }
    }

    prop_compose! {
        fn arb_item_filter()(allowed in any::<u32>()) -> ItemFilter {
            let allowed_set = match allowed % 3 {
                0 => HashSet::from([Item::new("Wood"), Item::new("Stone")]),
                1 => HashSet::from([Item::new("Iron ore"), Item::new("Coal")]),
                _ => HashSet::new(), // Empty set represents no filter
            };
            if allowed_set.is_empty() {
                ItemFilter::All
            } else {
                ItemFilter::Only(allowed_set)
            }
        }
    }

    proptest! {
        // Combine tests for adding, removing, and finding items
        #[test]
        fn test_inventory_operations(size in 1u32..10u32, items in prop::collection::vec(arb_product(), 1usize..=10usize)) {
            let mut inventory = Inventory::new(size);
            let items_to_test = items.into_iter().take(size as usize).collect::<Vec<_>>();

            // Add items to inventory
            for item in &items_to_test {
                inventory.add_item(item, 1);
            }

            // Test item finding
            for item in &items_to_test {
                prop_assert!(inventory.find_item(item.as_ref()).is_some());
            }

            // Remove items
            let items_to_remove = items_to_test.iter().map(|item| (item.clone(), 1)).collect::<Vec<_>>();
            prop_assert!(inventory.remove_items(&items_to_remove));

            // Test inventory state after operations
            for item in items_to_test {
                prop_assert!(!inventory.has_item(&item));
            }
        }

        // Test for adding items with a filter
        #[test]
        fn test_adding_items_with_filter(size in 1u32..10u32, filter in arb_item_filter()) {
            let allowed_items = match &filter {
                ItemFilter::All => vec![Item::new("Wood"), Item::new("Stone"), Item::new("Iron ore"), Item::new("Coal")],
                ItemFilter::Only(allowed) => allowed.iter().cloned().collect(),
            };

            let inventory = match &filter {
                ItemFilter::All => Inventory::new(size),
                ItemFilter::Only(_) => Inventory::new_with_filter(size, allowed_items.iter().cloned().collect()),
            };

            for item in allowed_items.iter() {
                let added = inventory.can_add_item(item);
                prop_assert!(added);
            }
        }

        // Test for overflow handling
        #[test]
        fn test_overflow_handling(size in 1u32..10u32) {
            let items = vec![Item::new("Wood"), Item::new("Stone"), Item::new("Iron ore"), Item::new("Coal")];
            let mut inventory = Inventory::new(size);
            let items_to_add = items.iter().cycle().take(items.len() * (MAX_STACK_SIZE as usize + 1)).cloned().collect::<Vec<_>>();

            let remainder = inventory.add_items(&items_to_add.iter().map(|item| (item.clone(), MAX_STACK_SIZE + 1)).collect::<Vec<_>>());
            prop_assert!(!remainder.is_empty(), "Remainder should not be empty when overflowing the inventory");
        }

        // Test for swapping items between two inventories
        #[test]
        fn test_inventory_items_consistency_after_swaps(inventory in arb_inventory(10), actions in prop::collection::vec(arb_action(), 1..10)) {
            let initial_item_counts = count_items_by_type(&inventory);

            let mut inventory_a = inventory;
            let mut inventory_b = Inventory::new(10);

            for (action, item) in actions {
                match action {
                    SwapAction::MoveToB => {
                        if let Some(stack) = inventory_a.try_take_item(&item, 1) {
                            inventory_b.add_stack(stack);
                        }
                    },
                    SwapAction::MoveToA => {
                        if let Some(stack) = inventory_b.try_take_item(&item, 1) {
                            inventory_a.add_stack(stack);
                        }
                    }
                }
            }

            let final_item_counts_a = count_items_by_type(&inventory_a);
            let final_item_counts_b = count_items_by_type(&inventory_b);

            for (item, &initial_count) in &initial_item_counts {
                let final_count = final_item_counts_a.get(item).unwrap_or(&0) + final_item_counts_b.get(item).unwrap_or(&0);
                prop_assert_eq!(initial_count, final_count, "Mismatch in item counts for {:?}", item);
            }
        }

        // Test for simulating inventory actions
        #[test]
        fn test_inventory_actions(actions in prop::collection::vec(arb_inventory_action(), 1..10)) {
            let (inventory, expected_state) = simulate_inventory_actions(&actions);

            let actual_state = count_items_by_type(&inventory);

            for (item, &expected_count) in &expected_state {
                let actual_count = actual_state.get(item).unwrap_or(&0);
                prop_assert_eq!(&expected_count, actual_count, "Mismatch for item {:?}", item);
            }
        }
    }

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
    fn can_add() {
        let inventory = Inventory::new(12);
        assert!(inventory.can_add(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20)]));
    }

    #[test]
    fn can_add_not_enough_space() {
        let inventory = Inventory::new(12);
        assert!(!inventory.can_add(&[(Item::new("Stone"), 10), (Item::new("Wood"), 20000)]));
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
    fn add_items_overflow_stack() {
        let mut inventory = Inventory::new(2);
        inventory.slots[1] = Some(Stack::new(Item::new("Stone furnace"), 1000));
        inventory.add_items(&[(Item::new("Stone furnace"), 1)]);
        assert!(inventory.slots.iter().filter(|s| s.is_some()).count() == 2,);
    }

    #[test]
    fn add_items_merge_stack_when_possible() {
        let mut inventory = Inventory::new(2);
        inventory.slots[1] = Some(Stack::new(Item::new("Stone"), 10));
        inventory.add_items(&[(Item::new("Stone"), 1)]);
        assert_eq!(inventory.slots[1], Some(Stack::new(Item::new("Stone"), 11)));
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

    mod state_machine_tests {
        use crate::{
            inventory::{Inventory, MAX_STACK_SIZE},
            types::Item,
        };

        use proptest::prelude::*;
        use proptest::test_runner::Config;
        use proptest_state_machine::{prop_state_machine, ReferenceStateMachine, StateMachineTest};
        use std::collections::HashMap;

        prop_compose! {
            fn arb_item()(name in "[a-zA-Z]{1,10}") -> Item {
                Item::new(name)
            }
        }

        prop_compose! {
            fn arb_items()(items in prop::collection::vec("[a-zA-Z]{1,10}", 1..=20)) -> Vec<Item> {
                items.into_iter().map(Item::new).collect()
            }
        }

        prop_state_machine! {
            #![proptest_config(Config {
                verbose: 1,
                .. Config::default()
            })]

            #[test]
            fn run_inventory_test(sequential 1..5 => Inventory);
        }

        pub struct InventoryStateMachine;

        #[derive(Clone, Debug)]
        pub enum Transition {
            AddItem(Item, u32),
            RemoveItem(Item, u32),
            CheckItem(Item),
            CanAdd(Vec<(Item, u32)>),
        }

        #[derive(Clone, Debug)]
        pub struct InventoryState {
            size: u32,
            items: HashMap<Item, u32>,
            item_pool: Vec<Item>,
            remainders: Vec<u32>,
            removals: Vec<bool>,
            checks: Vec<bool>,
            can_add: Vec<bool>,
        }

        impl ReferenceStateMachine for InventoryStateMachine {
            type State = InventoryState; // (Inventory size, Map of item to total count, Remainder)
            type Transition = Transition;

            fn init_state() -> BoxedStrategy<Self::State> {
                (1u32..=10u32, arb_items())
                    .prop_map(|(size, items)| InventoryState {
                        size,
                        items: HashMap::new(),
                        item_pool: items,
                        remainders: Vec::new(),
                        removals: Vec::new(),
                        checks: Vec::new(),
                        can_add: Vec::new(),
                    })
                    .boxed()
            }

            fn transitions(state: &Self::State) -> BoxedStrategy<Self::Transition> {
                let item_pool_strategy = Just(state.item_pool.clone());
                prop_oneof![
                    item_pool_strategy.clone().prop_flat_map(|items| {
                        let item_strategy = prop::sample::select(items);
                        (item_strategy, 1u32..=10000)
                            .prop_map(|(item, amount)| Transition::AddItem(item, amount))
                    }),
                    item_pool_strategy.clone().prop_flat_map(|items| {
                        let item_strategy = prop::sample::select(items);
                        (item_strategy, 1u32..=10000)
                            .prop_map(|(item, amount)| Transition::RemoveItem(item, amount))
                    }),
                    item_pool_strategy.clone().prop_flat_map(|items| {
                        let item_strategy = prop::sample::select(items);
                        item_strategy.prop_map(Transition::CheckItem)
                    }),
                    item_pool_strategy.prop_flat_map(|items| {
                        let item_strategy = prop::sample::select(items);
                        (1u32..=10000, item_strategy)
                            .prop_map(|(amount, item)| Transition::CanAdd(vec![(item, amount)]))
                    }),
                ]
                .boxed()
            }

            fn apply(mut state: Self::State, transition: &Self::Transition) -> Self::State {
                let InventoryState {
                    size,
                    items: stored_items,
                    remainders,
                    removals,
                    checks,
                    can_add,
                    ..
                } = &mut state;

                match transition {
                    Transition::AddItem(item, amount) => {
                        if amount == &0 {
                            return state;
                        }
                        let current_count = stored_items.get(item).copied().unwrap_or(0);
                        let mut total_count = current_count + amount;

                        let mut slots_needed = total_count / MAX_STACK_SIZE;
                        if total_count % MAX_STACK_SIZE != 0 {
                            slots_needed += 1; // Account for partial fill
                        }

                        let used_slots_by_other_items = stored_items
                            .iter()
                            .filter(|(i, _)| i != &item)
                            .map(|(_, &count)| (count + MAX_STACK_SIZE - 1) / MAX_STACK_SIZE)
                            .sum::<u32>();

                        let empty_slots = *size - used_slots_by_other_items;

                        if slots_needed <= empty_slots {
                            stored_items.insert(item.clone(), total_count);
                            remainders.push(0);
                        } else {
                            let overflow = total_count - empty_slots * MAX_STACK_SIZE;
                            total_count -= overflow;
                            stored_items.insert(item.clone(), total_count);
                            remainders.push(overflow);
                        }
                    }
                    Transition::RemoveItem(item, amount) => {
                        let current_count = stored_items.get(item).copied().unwrap_or(0);
                        if current_count < *amount {
                            removals.push(false);
                        } else {
                            stored_items.insert(item.clone(), current_count - amount);
                            stored_items.retain(|_, count| *count > 0);
                            removals.push(true);
                        }
                    }
                    Transition::CheckItem(item) => {
                        if stored_items.get(item).map_or(false, |f| *f > 0) {
                            checks.push(true);
                        } else {
                            checks.push(false);
                        }
                    }
                    Transition::CanAdd(items) => {
                        let mut scratch = stored_items.clone();
                        for (item, amount) in items {
                            let current_count = scratch.get(item).copied().unwrap_or(0);
                            scratch.insert(item.clone(), current_count + amount);
                        }

                        let new_slots_needed = scratch
                            .iter()
                            .map(|(_, &count)| (count + MAX_STACK_SIZE - 1) / MAX_STACK_SIZE)
                            .sum::<u32>();

                        can_add.push(new_slots_needed <= *size);
                    }
                }

                state
            }
        }

        impl StateMachineTest for Inventory {
            type SystemUnderTest = Self;
            type Reference = InventoryStateMachine;

            fn init_test(
                ref_state: &<Self::Reference as ReferenceStateMachine>::State,
            ) -> Self::SystemUnderTest {
                Inventory::new(ref_state.size)
            }

            fn apply(
                mut state: Self::SystemUnderTest,
                ref_state: &<Self::Reference as ReferenceStateMachine>::State,
                transition: Transition,
            ) -> Self::SystemUnderTest {
                match transition {
                    Transition::AddItem(item, amount) => {
                        let remainder = state.add_item(&item, amount);
                        let expected_remainder = *ref_state.remainders.last().unwrap();
                        assert_eq!(remainder, expected_remainder, "Remainder mismatch");
                    }
                    Transition::RemoveItem(item, amount) => {
                        let could_remove = state.remove_items(&[(item, amount)]);
                        assert_eq!(
                            could_remove,
                            *ref_state.removals.last().unwrap(),
                            "Removal mismatch"
                        );
                    }
                    Transition::CheckItem(item) => {
                        let has_item = state.has_item(&item);
                        assert_eq!(
                            has_item,
                            *ref_state.checks.last().unwrap(),
                            "Check mismatch"
                        );
                    }
                    Transition::CanAdd(items) => {
                        let can_add = state.can_add(&items);
                        assert_eq!(
                            can_add,
                            *ref_state.can_add.last().unwrap(),
                            "Can add mismatch"
                        );
                    }
                }

                state
            }

            fn check_invariants(
                state: &Self::SystemUnderTest,
                ref_state: &<Self::Reference as ReferenceStateMachine>::State,
            ) {
                // Check item counts
                for (item, &count) in &ref_state.items {
                    assert_eq!(state.num_items(item), count);
                }

                // Check that stack sizes never exceed max stack size
                for stack in state.slots.iter().flatten() {
                    assert!(stack.amount <= MAX_STACK_SIZE);
                }

                // Check amount of slots used per item
                for (item, &count) in &ref_state.items {
                    let ref_slots_used = (count + MAX_STACK_SIZE - 1) / MAX_STACK_SIZE;
                    let slots_used = state
                        .slots
                        .iter()
                        .filter(|s| {
                            if let Some(stack) = s {
                                stack.item == *item
                            } else {
                                false
                            }
                        })
                        .count();
                    assert_eq!(slots_used, ref_slots_used as usize);
                }

                // Check that only one stack per item is < MAX_STACK_SIZE
                let partial_slot_count = state
                    .slots
                    .iter()
                    .flatten()
                    .filter(|s| s.amount < MAX_STACK_SIZE)
                    .fold(HashMap::new(), |mut map, stack| {
                        *map.entry(stack.item.clone()).or_insert(0) += 1;
                        map
                    });
                assert!(partial_slot_count.values().all(|&count| count == 1));
            }
        }
    }
}
