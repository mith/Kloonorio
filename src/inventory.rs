use bevy::prelude::*;

use crate::types::Product;

const MAX_STACK_SIZE: u32 = 1000;

#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stack {
    pub resource: Product,
    pub amount: u32,
}

impl Stack {
    pub fn new(resource: Product, amount: u32) -> Self {
        Self { resource, amount }
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
}

impl Inventory {
    pub fn new(size: u32) -> Self {
        Self {
            slots: (0..size).map(|_| None).collect(),
        }
    }

    /// Return true if the inventory has enough space for the items
    pub fn can_add(&self, items: &[(Product, u32)]) -> bool {
        let mut slots = self.slots.clone();
        let items = items.to_vec();
        for (item_resource, mut item_amount) in items {
            let mut added = false;
            for slot in slots.iter_mut() {
                if let Some(stack) = slot {
                    if stack.resource == item_resource {
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
    pub fn add_items(&mut self, items: &[(Product, u32)]) -> Vec<(Product, u32)> {
        let mut remainder = Vec::new();
        for (resource, amount) in items {
            let mut amount = *amount;

            // First iterate over existing stacks
            for stack in self.slots.iter_mut().flatten() {
                if stack.resource == *resource {
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
                    resource: resource.clone(),
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

    pub fn add_item(&mut self, resource: Product, amount: u32) -> Vec<(Product, u32)> {
        self.add_items(&[(resource, amount)])
    }

    pub fn has_items(&self, items: &[(Product, u32)]) -> bool {
        for (resource, amount) in items {
            let mut amount = *amount;
            for slot in self.slots.iter() {
                if amount == 0 {
                    break;
                }
                if let Some(stack) = slot {
                    if stack.resource == *resource {
                        if stack.amount >= amount {
                            amount = 0;
                        } else {
                            amount -= stack.amount;
                        }
                    }
                }
            }
            if amount > 0 {
                return false;
            }
        }
        true
    }

    /// Removes all items atomically, returning true on success
    pub fn remove_items(&mut self, items: &[(Product, u32)]) -> bool {
        if !self.has_items(items) {
            return false;
        }

        for (resource, amount) in items {
            let mut amount = *amount;
            for slot in self.slots.iter_mut() {
                if amount == 0 {
                    break;
                }
                if let Some(stack) = slot {
                    if stack.resource == *resource {
                        if stack.amount >= amount {
                            stack.amount -= amount;
                            amount = 0;
                        } else {
                            amount -= stack.amount;
                            stack.amount = 0;
                        }
                        if stack.amount == 0 {
                            *slot = None;
                        }
                    }
                }
            }
        }
        true
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
            info!("Moving source stack to target slot");
            *target_slot = Some(source_stack.clone());
            *source_slot = None;
        }
    }
}

pub fn drop_within_inventory(inventory: &mut Inventory, source_slot: usize, target_slot: usize) {
    let span = info_span!("drop_within_inventory", source_slot, target_slot);
    let _enter = span.enter();
    if let Some(mut source_stack) = inventory.slots.get(source_slot).cloned().flatten() {
        if let Some(mut target_stack) = inventory.slots.get(target_slot).cloned().flatten() {
            transfer_between_stacks(&mut source_stack, &mut target_stack);
            inventory.slots[target_slot] = Some(target_stack);
            inventory.slots[source_slot] = {
                if source_stack.amount > 0 {
                    info!(source_stack = ?source_stack, "Keeping source stack");
                    Some(source_stack)
                } else {
                    info!("Dropping source stack");
                    None
                }
            };
        } else {
            info!("Moving source stack to target slot");
            inventory.slots[target_slot] = Some(source_stack.clone());
            inventory.slots[source_slot] = None;
        }
    } else {
        error!("Source slot is empty");
    }
}

pub fn transfer_between_stacks(source_stack: &mut Stack, target_stack: &mut Stack) {
    if source_stack == target_stack {
        return;
    }
    if target_stack.resource == source_stack.resource {
        info!("Adding source stack to target stack");
        let remainder = target_stack.add(source_stack.amount);
        source_stack.amount = remainder;
    } else {
        info!("Swapping stacks");
        std::mem::swap(source_stack, target_stack);
    }
}
#[derive(Component)]
pub struct Source;

#[derive(Component)]
pub struct Output;

#[derive(Component)]
pub struct Fuel;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn has_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        assert!(inventory.has_items(&[
            (Product::Intermediate("Stone".into()), 5),
            (Product::Intermediate("Wood".into()), 10)
        ]));
        assert!(!inventory.has_items(&[
            (Product::Intermediate("Stone".into()), 5),
            (Product::Intermediate("Wood".into()), 30)
        ]));
    }

    #[test]
    fn remove_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        inventory.remove_items(&[
            (Product::Intermediate("Stone".into()), 5),
            (Product::Intermediate("Wood".into()), 10),
        ]);
        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Product::Intermediate("Stone".into()), 5))
        );
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Product::Intermediate("Wood".into()), 10))
        );
    }

    #[test]
    fn remove_items_empty() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        inventory.remove_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        assert!(inventory.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn remove_items_not_enough() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        assert!(!inventory.remove_items(&[
            (Product::Intermediate("Stone".into()), 5),
            (Product::Intermediate("Wood".into()), 30)
        ]));
        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Product::Intermediate("Stone".into()), 10))
        );
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Product::Intermediate("Wood".into()), 20))
        );
    }
    #[test]
    fn add_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Product::Intermediate("Stone".into()), 10))
        );
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Product::Intermediate("Wood".into()), 20))
        );
    }

    #[test]
    fn add_items_remainder() {
        let mut inventory = Inventory::new(1);
        let remainder = inventory.add_items(&[
            (Product::Intermediate("Stone".into()), 10),
            (Product::Intermediate("Wood".into()), 20),
        ]);
        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Product::Intermediate("Stone".into()), 10))
        );
        assert_eq!(remainder, vec![(Product::Intermediate("Wood".into()), 20)]);
    }

    #[test]
    fn add_items_stack() {
        let mut inventory = Inventory::new(2);
        inventory.slots[1] = Some(Stack::new(Product::Structure("Stone furnace".into()), 10));
        inventory.add_items(&[(Product::Structure("Stone furnace".into()), 1)]);
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Product::Structure("Stone furnace".into()), 11))
        );
    }

    #[test]
    fn transfer_between_stacks_swap() {
        let mut source_stack = Stack::new(Product::Intermediate("Stone".into()), 10);
        let mut target_stack = Stack::new(Product::Intermediate("Iron ore".into()), 20);

        transfer_between_stacks(&mut source_stack, &mut target_stack);

        assert_eq!(
            source_stack,
            Stack::new(Product::Intermediate("Iron ore".into()), 20)
        );
        assert_eq!(
            target_stack,
            Stack::new(Product::Intermediate("Stone".into()), 10)
        );
    }

    #[test]
    fn transfer_between_stacks_same() {
        let mut source_stack = Stack::new(Product::Intermediate("Stone".into()), 10);
        let mut target_stack = Stack::new(Product::Intermediate("Stone".into()), 20);

        transfer_between_stacks(&mut source_stack, &mut target_stack);

        assert_eq!(
            source_stack,
            Stack::new(Product::Intermediate("Stone".into()), 0)
        );
        assert_eq!(
            target_stack,
            Stack::new(Product::Intermediate("Stone".into()), 30)
        );
    }

    #[test]
    fn transfer_between_slots_swap() {
        let mut source_slot = Some(Stack::new(Product::Intermediate("Stone".into()), 10));
        let mut target_slot = Some(Stack::new(Product::Intermediate("Iron ore".into()), 20));

        transfer_between_slots(&mut source_slot, &mut target_slot);

        assert_eq!(
            source_slot,
            Some(Stack::new(Product::Intermediate("Iron ore".into()), 20))
        );
        assert_eq!(
            target_slot,
            Some(Stack::new(Product::Intermediate("Stone".into()), 10))
        );
    }

    #[test]
    fn transfer_between_slots_merge_stacks() {
        let mut source_slot = Some(Stack::new(Product::Intermediate("Stone".into()), 10));
        let mut target_slot = Some(Stack::new(Product::Intermediate("Stone".into()), 20));

        transfer_between_slots(&mut source_slot, &mut target_slot);

        assert_eq!(source_slot, None);
        assert_eq!(
            target_slot,
            Some(Stack::new(Product::Intermediate("Stone".into()), 30))
        );
    }

    #[test]
    fn transfer_between_slots_empty() {
        let mut source_slot = Some(Stack::new(Product::Intermediate("Stone".into()), 10));
        let mut target_slot = None;

        transfer_between_slots(&mut source_slot, &mut target_slot);

        assert_eq!(source_slot, None);
        assert_eq!(
            target_slot,
            Some(Stack::new(Product::Intermediate("Stone".into()), 10))
        );
    }

    #[test]
    fn drop_within_inventory_swap() {
        let mut inventory = Inventory::new(12);

        inventory.slots[0] = Some(Stack::new(Product::Intermediate("Stone".into()), 10));
        inventory.slots[1] = Some(Stack::new(Product::Intermediate("Iron ore".into()), 20));

        drop_within_inventory(&mut inventory, 1, 0);

        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Product::Intermediate("Iron ore".into()), 20))
        );
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Product::Intermediate("Stone".into()), 10))
        );
    }
}
