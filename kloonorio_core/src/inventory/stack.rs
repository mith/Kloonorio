use bevy::{ecs::component::Component, reflect::Reflect};

use crate::item::Item;

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
