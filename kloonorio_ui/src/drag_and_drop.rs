use bevy::prelude::*;

use crate::inventory_grid::{Hand, InventoryIndex, SlotEvent};
use kloonorio_core::{
    inventory::{drop_within_inventory, transfer_between_slots, Inventory, Slot, Stack},
    player::Player,
};

pub fn drop_system(
    mut hand_query: Query<&mut Hand, With<Player>>,
    mut slot_events: EventReader<SlotEvent>,
    mut inventories_query: Query<&mut Inventory>,
) {
    for event @ SlotEvent::Clicked(drop) in slot_events.read() {
        let span = info_span!("Handling drop event", ?event);
        let _enter = span.enter();
        for mut hand in hand_query.iter_mut() {
            info_span!("hand", hand = ?hand);
            if let Some(item_in_hand) = hand.get_item() {
                if item_in_hand.entity == drop.entity {
                    if item_in_hand.slot == drop.slot {
                        hand.clear();
                    } else {
                        let mut inventory = inventories_query.get_mut(item_in_hand.entity).unwrap();
                        drop_within_inventory(&mut inventory, item_in_hand.slot, drop.slot);
                    }
                } else if let Ok([mut source_inventory, mut target_inventory]) =
                    inventories_query.get_many_mut([item_in_hand.entity, drop.entity])
                {
                    let source_slot: &mut Slot =
                        source_inventory.slots.get_mut(item_in_hand.slot).unwrap();
                    let target_slot: &mut Slot = target_inventory.slots.get_mut(drop.slot).unwrap();
                    transfer_between_slots(source_slot, target_slot);
                    hand.reset_rotation();
                } else {
                    error!("Could not get inventories");
                }
            } else if let Ok(inventory) = inventories_query.get_mut(drop.entity) {
                if inventory.slots[drop.slot].is_some() {
                    // No item it hand, but there is an item in the slot, pick it up
                    let inventory_index = InventoryIndex::new(drop.entity, drop.slot);
                    debug!(inventory_index = ?inventory_index, "Putting clicked slot in hand");
                    hand.set_item(drop.entity, drop.slot);
                }
            }

            // If the hand contains an InventoryIndex pointing to an empty slot, empty the hand
            if let Some(item_in_hand) = hand.get_item() {
                let inventory = inventories_query.get(item_in_hand.entity).unwrap();
                let slot: &Option<Stack> = &inventory.slots[item_in_hand.slot];
                if slot.is_none() {
                    hand.clear();
                    debug!("Emptied hand");
                }
            }
        }
    }
}

pub fn clear_hand(
    keyboard_input: Res<Input<KeyCode>>,
    mut hand_query: Query<&mut Hand, With<Player>>,
) {
    if keyboard_input.just_pressed(KeyCode::Apostrophe) {
        for mut hand in hand_query.iter_mut() {
            hand.clear();
        }
    }
}

#[cfg(test)]
mod test {
    use crate::inventory_grid::InventoryIndex;
    use kloonorio_core::{inventory::Stack, item::Item};

    use super::*;
    use bevy::utils::HashMap;
    use proptest::prelude::*;

    prop_compose! {
    fn arb_item()(product in any::<u32>()) -> Item {
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
    fn arb_inventory(size: u32)(items in prop::collection::vec(arb_item(), 1..(size as usize))) -> Inventory {
        let mut inventory = Inventory::new(size);
        for item in items {
            inventory.add_item(&item, 1);
        }
            inventory
    }
    }

    proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]
        #[test]
        fn drop_system_no_duplication(inventory in arb_inventory(10), source_slot in 0..4u32, target_slot in 0..10u32) {
        let mut app = App::new();



        let mut input = Input::<MouseButton>::default();
        input.press(MouseButton::Left);
        app.insert_resource(input);

        let count = inventory.slots
            .iter()
                .flatten()
                .fold(HashMap::new(), |mut acc, stack| {
                    if acc.contains_key(&stack.item) {
                        *acc.get_mut(&stack.item).unwrap() += stack.amount;
                    } else {
                        acc.insert(stack.item.clone(), stack.amount);
                    }
                    acc
            });

        let player_id = app
            .world
            .spawn((Player, inventory))
                .id();

        let inventory_id = player_id;

        app.world.get_entity_mut(player_id)
                .unwrap()
                .insert(Hand::new(inventory_id, source_slot as usize));

        app.add_event::<SlotEvent>();

        app.world.resource_mut::<Events<SlotEvent>>().send(SlotEvent::clicked(inventory_id, target_slot as usize));

        app.update();

        let post_count = app
            .world
            .get::<Inventory>(inventory_id)
                .unwrap()
                .slots
            .iter()
                .flatten()
                .fold(HashMap::new(), |mut acc, stack| {
                if acc.contains_key(&stack.item) {
                    *acc.get_mut(&stack.item).unwrap() += stack.amount;
                } else {
                    acc.insert(stack.item.clone(), stack.amount);
                }
                    acc
            });

        for (item, amount) in post_count {
            assert_eq!(count.get(&item), Some(&amount));
        }
        }
    }

    #[test]
    fn drop_system_put_in_hand() {
        let mut app = App::new();

        let mut inventory = Inventory::new(10);

        inventory.add_item(&Item::new("Wood"), 1);

        let player_id = app.world.spawn((Player, inventory)).id();

        let hand = Hand::default();

        app.world.get_entity_mut(player_id).unwrap().insert(hand);

        app.add_event::<SlotEvent>();
        app.world
            .resource_mut::<Events<SlotEvent>>()
            .send(SlotEvent::clicked(player_id, 0));

        app.add_systems(Update, drop_system);

        app.update();

        assert_eq!(
            app.world.get::<Hand>(player_id).unwrap().get_item(),
            Some(InventoryIndex::new(player_id, 0))
        );
    }

    #[test]
    fn drop_system_to_empty_clear_hand() {
        let mut app = App::new();

        let mut inventory = Inventory::new(10);

        inventory.add_item(&Item::new("Wood"), 1);

        let player_id = app.world.spawn((Player, inventory)).id();

        let hand = Hand::new(player_id, 0);

        app.world.get_entity_mut(player_id).unwrap().insert(hand);

        app.add_event::<SlotEvent>();
        app.world
            .resource_mut::<Events<SlotEvent>>()
            .send(SlotEvent::clicked(player_id, 1));

        app.add_systems(Update, drop_system);

        app.update();

        assert_eq!(app.world.get::<Hand>(player_id).unwrap().get_item(), None);
    }

    #[test]
    fn drop_system_same_slot() {
        let mut app = App::new();

        let mut inventory = Inventory::new(10);

        inventory.add_item(&Item::new("Wood"), 1);

        let player_id = app.world.spawn((Player, inventory)).id();

        let hand = Hand::new(player_id, 0);

        app.world.get_entity_mut(player_id).unwrap().insert(hand);

        app.add_event::<SlotEvent>();
        app.world
            .resource_mut::<Events<SlotEvent>>()
            .send(SlotEvent::clicked(player_id, 0));

        app.add_systems(Update, drop_system);

        app.update();

        assert_eq!(app.world.get::<Hand>(player_id).unwrap().get_item(), None);
    }

    #[test]
    fn drop_system_same_product() {
        let mut app = App::new();

        let mut inventory = Inventory::new(10);

        inventory.slots[0] = Some(Stack::new(Item::new("Wood"), 1));
        inventory.slots[1] = Some(Stack::new(Item::new("Wood"), 1));

        let player_id = app.world.spawn((Player, inventory)).id();

        let hand = Hand::new(player_id, 0);

        app.world.get_entity_mut(player_id).unwrap().insert(hand);

        app.add_event::<SlotEvent>();
        app.world
            .resource_mut::<Events<SlotEvent>>()
            .send(SlotEvent::clicked(player_id, 1));

        app.add_systems(Update, drop_system);

        app.update();

        assert_eq!(app.world.get::<Hand>(player_id).unwrap().get_item(), None);
        assert_eq!(
            app.world
                .get::<Inventory>(player_id)
                .unwrap()
                .slots
                .get(1)
                .cloned()
                .unwrap()
                .unwrap()
                .amount,
            2
        );
    }
}
