use bevy::ecs::{
    entity::Entity,
    query::Without,
    system::{Query, SystemParam},
};

use crate::{
    inventory::{Inventory, Output, Stack},
    structure_components::transport_belt::TransportBelt,
    tile_occupants::TileOccupants,
};

#[derive(SystemParam)]
pub struct DropParams<'w, 's> {
    tile_occupants_query: Query<'w, 's, &'static TileOccupants>,
    inventories_query: Query<'w, 's, &'static mut Inventory, Without<Output>>,
    belts_query: Query<'w, 's, &'static mut TransportBelt>,
}

impl DropParams<'_, '_> {
    pub fn can_drop_stack_at_tile(&self, stack: &Stack, tile: Entity) -> bool {
        self.tile_occupants_query
            .get(tile)
            .ok()
            .map_or(false, |occupants| {
                occupants.iter().any(|&entity| {
                    self.inventories_query
                        .get(entity)
                        .map_or(false, |inventory| inventory.can_add_stack(stack))
                        || self
                            .belts_query
                            .get(entity)
                            .map_or(false, |belt| belt.can_add(1))
                })
            })
    }

    pub fn drop_stack_at_tile(&mut self, stack: &Stack, tile: Entity) -> bool {
        self.tile_occupants_query
            .get(tile)
            .ok()
            .map_or(false, |occupants| {
                occupants.iter().any(|&entity| {
                    if let Ok(mut inventory) = self.inventories_query.get_mut(entity) {
                        if inventory.can_add_stack(stack) {
                            inventory.add_stack(stack.clone());
                            return true;
                        }
                    }
                    if let Ok(mut belt) = self.belts_query.get_mut(entity) {
                        if belt.add(1, stack.item.clone()) {
                            return true;
                        }
                    }
                    false
                })
            })
    }
}
