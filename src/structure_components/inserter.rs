use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::{Collider, RapierContext};

use crate::{
    inventory::{Inventory, ItemFilter, Stack},
    types::{Item, Powered, Working},
    util::{
        find_entities_on_position, get_inventory_child_mut, FuelInventoryQuery, Inventories,
        InventoryType,
    },
};

use super::{burner::Burner, transport_belt::TransportBelt};

pub struct InserterPlugin;

impl Plugin for InserterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                (
                    inserter_planner,
                    apply_deferred.in_set(InserterFlush),
                    inserter_tick,
                    animate_arm_position,
                )
                    .chain(),
                burner_inserter_tick,
            ),
        )
        .register_type::<Inserter>();
    }
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Reflect, SystemSet)]
struct InserterFlush;

#[derive(Component, Debug, Reflect)]
pub struct Inserter {
    holding: Option<Stack>,
    capacity: u32,
    arm_position: f32, // -1 = Pickup, 0 = Self, 1 = Dropoff
    target_arm_position: f32,
    dropoff_location_entity: Entity,
    pickup_location_entity: Entity,
    current_action: Option<InserterAction>,
    speed: f32,
}

impl Inserter {
    pub fn new(
        speed: f32,
        capacity: u32,
        dropoff_location_entity: Entity,
        pickup_location_entity: Entity,
    ) -> Self {
        Inserter {
            holding: None,
            capacity,
            arm_position: 0.,
            target_arm_position: 0.,
            dropoff_location_entity,
            pickup_location_entity,
            current_action: None,
            speed,
        }
    }
}

#[derive(Component)]
pub struct Pickup;

#[derive(Component)]
pub struct Dropoff;

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
enum InserterTargetType {
    Belt(Entity),
    Inventory(Entity),
    ItemOnGround(Entity),
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
enum InserterTargetItem {
    Any,
    Filter(Vec<Item>),
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct DropoffRequest {
    target_type: InserterTargetType,
    target_item: InserterTargetItem,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct AvailablePickup {
    target_type: InserterTargetType,
    target_item: Item,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct InserterAction {
    pickup: Option<InserterTargetType>,
    dropoff: InserterTargetType,
    item: Item,
}

// A cookie for anyone who can figure out how to make all the find_* functions return iterators with valid lifetimes

fn find_inventory_pickups_for_entity<'a>(
    entity: Entity,
    inventories: &'a Inventories,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 'a {
    [InventoryType::Output, InventoryType::Storage]
        .iter()
        .flat_map(move |&inventory_type| {
            inventories
                .get_inventory_component(entity, inventory_type)
                .into_iter()
                .flat_map(move |inventory| {
                    inventory.slots.iter().filter_map(move |slot| {
                        slot.as_ref().and_then(|stack| {
                            let is_target_item = match target_item {
                                InserterTargetItem::Any => true,
                                InserterTargetItem::Filter(ref filter) => {
                                    filter.contains(&stack.item)
                                }
                            };

                            if is_target_item {
                                Some(AvailablePickup {
                                    target_type: InserterTargetType::Inventory(entity),
                                    target_item: stack.item.clone(),
                                })
                            } else {
                                None
                            }
                        })
                    })
                })
        })
}

fn find_belt_pickups_for_entity<'a>(
    entity: Entity,
    belts_query: &'a Query<&TransportBelt>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 'a {
    belts_query
        .get(entity)
        .ok()
        .into_iter()
        .flat_map(move |belt| {
            belt.slot(1).unwrap().as_ref().and_then(|item| {
                let is_target_item = match target_item {
                    InserterTargetItem::Any => true,
                    InserterTargetItem::Filter(ref filter) => filter.contains(item),
                };

                if is_target_item {
                    Some(AvailablePickup {
                        target_type: InserterTargetType::Belt(entity),
                        target_item: item.clone(),
                    })
                } else {
                    None
                }
            })
        })
}

fn find_pickups(
    inserter: &Inserter,
    inventories: &Inventories,
    belts_query: &Query<&TransportBelt>,
    rapier_context: &Res<RapierContext>,
    collider_query: &Query<(&Collider, &GlobalTransform)>,
    target_item: &InserterTargetItem,
) -> Vec<AvailablePickup> {
    collider_query
        .get(inserter.pickup_location_entity)
        .ok()
        .map(|(_c, t)| find_entities_on_position(rapier_context, t.translation().xy(), None))
        .into_iter()
        .flatten()
        .flat_map(move |entity| {
            let inventory_pickups =
                find_inventory_pickups_for_entity(entity, inventories, target_item);
            let belt_pickups = find_belt_pickups_for_entity(entity, belts_query, target_item);
            inventory_pickups.chain(belt_pickups)
        })
        .collect()
}

fn find_inventory_dropoffs_for_entity<'a>(
    entity: Entity,
    inventories: &'a Inventories<'_, '_>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = DropoffRequest> + 'a {
    [
        InventoryType::Fuel,
        InventoryType::Source,
        InventoryType::Storage,
    ]
    .iter()
    .filter_map(move |&inventory_type| {
        inventories
            .get_inventory_component(entity, inventory_type)
            .and_then(|inventory| match inventory.allowed_items {
                ItemFilter::All => Some(DropoffRequest {
                    target_type: InserterTargetType::Inventory(entity),
                    target_item: InserterTargetItem::Any,
                }),
                ItemFilter::Only(ref allowed_items) => {
                    let union: Vec<_> = allowed_items
                        .iter()
                        .filter(|&i| match target_item {
                            InserterTargetItem::Any => true,
                            InserterTargetItem::Filter(ref filter) => filter.contains(i),
                        })
                        .cloned()
                        .collect();

                    if !union.is_empty() {
                        Some(DropoffRequest {
                            target_type: InserterTargetType::Inventory(entity),
                            target_item: InserterTargetItem::Filter(union),
                        })
                    } else {
                        None
                    }
                }
            })
    })
}

fn find_belt_dropoffs_for_entity<'a>(
    entity: Entity,
    belts_query: &'a Query<'_, '_, &TransportBelt>,
) -> impl Iterator<Item = DropoffRequest> + 'a {
    belts_query
        .get(entity)
        .ok()
        .into_iter()
        .filter_map(move |belt| {
            if belt.can_add(1) {
                Some(DropoffRequest {
                    target_type: InserterTargetType::Belt(entity),
                    target_item: InserterTargetItem::Any,
                })
            } else {
                None
            }
        })
}

fn find_dropoffs(
    inserter: &Inserter,
    inventories: &Inventories,
    belts_query: &Query<&TransportBelt>,
    collider_query: &Query<(&Collider, &GlobalTransform)>,
    rapier_context: &Res<RapierContext>,
    target_item: &InserterTargetItem,
) -> Vec<DropoffRequest> {
    collider_query
        .get(inserter.dropoff_location_entity)
        .ok()
        .map(|(_c, t)| find_entities_on_position(rapier_context, t.translation().xy(), None))
        .into_iter()
        .flatten()
        .flat_map(move |entity| {
            let inventory_dropoffs =
                find_inventory_dropoffs_for_entity(entity, inventories, target_item);
            let belt_dropoffs = find_belt_dropoffs_for_entity(entity, belts_query);

            inventory_dropoffs.chain(belt_dropoffs)
        })
        .collect()
}

fn plan_inserter_action(
    inserter: &Inserter,
    inventories: &Inventories,
    collider_query: &Query<(&Collider, &GlobalTransform)>,
    belts_query: &Query<&TransportBelt>,
    rapier_context: &Res<RapierContext>,
) -> Option<InserterAction> {
    let target_item = inserter
        .holding
        .as_ref()
        .map(|stack| InserterTargetItem::Filter(vec![stack.item.clone()]))
        .unwrap_or(InserterTargetItem::Any);

    let dropoffs = find_dropoffs(
        inserter,
        inventories,
        belts_query,
        collider_query,
        rapier_context,
        &target_item,
    );
    if let Some(holding) = inserter.holding.as_ref() {
        dropoffs
            .iter()
            .find(|d| match d.target_item {
                InserterTargetItem::Any => true,
                InserterTargetItem::Filter(ref filter) => filter.contains(&holding.item),
            })
            .map(|dropoff| InserterAction {
                pickup: None,
                dropoff: dropoff.target_type.clone(),
                item: holding.item.clone(),
            })
    } else {
        // Iterate through each dropoff and try to find a matching pickup
        dropoffs.iter().find_map(|dropoff| {
            find_pickups(
                inserter,
                inventories,
                belts_query,
                rapier_context,
                collider_query,
                &dropoff.target_item,
            )
            .iter()
            .find(|pickup| match dropoff.target_item {
                InserterTargetItem::Any => true,
                InserterTargetItem::Filter(ref filter) => filter.contains(&pickup.target_item),
            })
            .map(|pickup| InserterAction {
                pickup: Some(pickup.target_type.clone()),
                dropoff: dropoff.target_type.clone(),
                item: pickup.target_item.clone(),
            })
        })
    }
}

fn check_inserter_action_valid<'w, 's, 'a>(
    inserter: &'a Inserter,
    inventories: &'a Query<&Inventory>,
    belts_query: &'a Query<'w, 's, &TransportBelt>,
    rapier_context: &'a Res<'w, RapierContext>,
    collider_query: &'a Query<'w, 's, (&Collider, &GlobalTransform)>,
    action: &'a InserterAction,
) -> bool {
    let holding_valid = inserter
        .holding
        .as_ref()
        .map(|stack| action.item == stack.item)
        .unwrap_or(true);

    if !holding_valid {
        return false;
    }

    let dropoff_valid = match action.dropoff {
        InserterTargetType::Belt(entity) => belts_query
            .get(entity)
            .ok()
            .map(|belt| belt.can_add(1))
            .unwrap_or(false),
        InserterTargetType::Inventory(entity) => {
            let inventory_in_range = collider_query
                .get(inserter.dropoff_location_entity)
                .ok()
                .map(|(_c, t)| {
                    find_entities_on_position(rapier_context, t.translation().xy(), None)
                })
                .into_iter()
                .flatten()
                .any(|e| e == entity);

            if !inventory_in_range {
                return false;
            }

            let space_in_inventory = inventories
                .get(entity)
                .map_or(false, |inventory| inventory.can_add_item(&action.item));
            space_in_inventory
        }
        InserterTargetType::ItemOnGround(_entity) => {
            // TODO: Check if the ground is clear
            true
        }
    };

    if !dropoff_valid {
        return false;
    }

    if inserter.holding.is_some() {
        return true;
    }

    let pickup_valid = action.pickup.as_ref().map_or(true, |pickup| {
        match pickup {
            InserterTargetType::Belt(entity) => belts_query
                .get(*entity)
                .ok()
                .map(|belt| belt.slot(1).unwrap().is_none())
                .unwrap_or(false),
            InserterTargetType::Inventory(entity) => inventories
                .get(*entity)
                .map_or(false, |inventory| inventory.has_item(&action.item)),
            InserterTargetType::ItemOnGround(_entity) => {
                // TODO: Check if the ground is clear
                true
            }
        }
    });

    pickup_valid
}

fn inserter_planner(
    mut commands: Commands,
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter), With<Powered>>,
    collider_query: Query<(&Collider, &GlobalTransform)>,
    rapier_context: Res<RapierContext>,
    mut inventories_set: ParamSet<(Inventories, Query<&Inventory>)>,
    belts_query: Query<&TransportBelt>,
) {
    for (inserter_entity, _inserter_transform, mut inserter) in &mut inserter_query {
        let span = info_span!("Inserter planner", inserter = ?inserter_entity);
        let _enter = span.enter();

        {
            let new_action_needed =
                inserter
                    .current_action
                    .as_ref()
                    .map_or(true, |current_action| {
                        !check_inserter_action_valid(
                            &inserter,
                            &inventories_set.p1(),
                            &belts_query,
                            &rapier_context,
                            &collider_query,
                            current_action,
                        )
                    });

            if new_action_needed {
                trace!("Planning new action");
                let new_action = plan_inserter_action(
                    &inserter,
                    &inventories_set.p0(),
                    &collider_query,
                    &belts_query,
                    &rapier_context,
                );
                inserter.target_arm_position = if inserter.holding.is_some() {
                    1.0
                } else {
                    -1.0
                };
                debug!(target_arm_position = ?inserter.target_arm_position);
                if new_action.is_some() {
                    commands.entity(inserter_entity).insert(Working);
                    info!(action=?new_action, "New action planned");
                } else {
                    debug!("No action planned");
                    commands.entity(inserter_entity).remove::<Working>();
                }
                inserter.current_action = new_action;
            }
        }
    }
}

fn inserter_tick(
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter), (With<Powered>, With<Working>)>,
    time: Res<Time<Fixed>>,
    mut inventories: Query<&mut Inventory>,
    mut belts_query: Query<&mut TransportBelt>,
) {
    for (inserter_entity, _inserter_transform, mut inserter) in &mut inserter_query {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        // Move towards the target location
        inserter.arm_position +=
            inserter.speed * time.delta_seconds() * inserter.target_arm_position.signum();
        inserter.arm_position = inserter.arm_position.clamp(-1.0, 1.0);

        if (inserter.arm_position - inserter.target_arm_position).abs() < 0.01 {
            if let Some(action) = inserter.current_action.clone() {
                if let Some(stack) = inserter.holding.take() {
                    // Dropoff
                    match action.dropoff {
                        InserterTargetType::Belt(entity) => {
                            let mut belt = belts_query.get_mut(entity).unwrap();
                            belt.add(1, stack.item);
                        }
                        InserterTargetType::Inventory(entity) => {
                            let mut inventory = inventories.get_mut(entity).unwrap();
                            inventory.add_stack(stack);
                        }
                        InserterTargetType::ItemOnGround(_entity) => {
                            // TODO: Implement dropping items on the ground
                        }
                    }
                    inserter.current_action = None;
                } else {
                    // Pickup
                    match action.pickup.unwrap() {
                        InserterTargetType::Belt(entity) => {
                            let mut belt = belts_query.get_mut(entity).unwrap();
                            let stack = belt.slot_mut(1).unwrap().take().unwrap();
                            inserter.holding = Some(Stack {
                                item: stack,
                                amount: 1,
                            });
                        }
                        InserterTargetType::Inventory(entity) => {
                            let mut inventory = inventories.get_mut(entity).unwrap();
                            let stack = inventory.try_take_item(&action.item, inserter.capacity);
                            inserter.holding = stack;
                        }
                        InserterTargetType::ItemOnGround(_entity) => {
                            // TODO: Implement picking up items from the ground
                        }
                    }
                }
            }
        }
    }
}

fn animate_arm_position(
    inserter_query: Query<(&GlobalTransform, &Inserter)>,
    transform_query: Query<&GlobalTransform>,
    mut gizmos: Gizmos,
) {
    for (inserter_transform, inserter) in &mut inserter_query.iter() {
        let span = info_span!("Animate arm position", inserter = ?inserter);
        let _enter = span.enter();

        let arm_position = inserter.arm_position;
        let inserter_location = inserter_transform.translation().xy();
        let pickup_location = transform_query
            .get(inserter.pickup_location_entity)
            .unwrap()
            .translation()
            .xy();
        let dropoff_location = transform_query
            .get(inserter.dropoff_location_entity)
            .unwrap()
            .translation()
            .xy();

        let arm_position = pickup_location.lerp(dropoff_location, (arm_position + 1.0) / 2.0);

        gizmos.circle_2d(inserter_location, 1., Color::YELLOW);
        gizmos.circle_2d(arm_position, 1., Color::YELLOW);
        gizmos.line_2d(inserter_location, arm_position, Color::YELLOW);
    }
}

fn burner_inserter_tick(
    mut burner_inserter_query: Query<(Entity, &mut Inserter, &Children), With<Burner>>,
    mut fuel_query: Query<FuelInventoryQuery>,
) {
    for (inserter_entity, mut inserter, inserter_children) in &mut burner_inserter_query {
        let span = info_span!("Burner inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        if let Some(stack) = inserter.holding.as_ref() {
            let mut fuel_inventory = get_inventory_child_mut(inserter_children, &mut fuel_query).1;
            if stack.item == Item::new("Coal")
                && !fuel_inventory.has_items(&[(Item::new("Coal"), 2)])
            {
                debug!("Taking fuel from hand to refuel");
                fuel_inventory.add_stack(stack.to_owned());
                inserter.holding = None;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use bevy::{
        app::{App, Plugin, Update},
        asset::AssetPlugin,
        ecs::system::{Query, Res},
        hierarchy::BuildWorldChildren,
        render::{
            settings::{RenderCreation, WgpuSettings},
            texture::ImagePlugin,
            RenderPlugin,
        },
        scene::ScenePlugin,
        time::TimePlugin,
        transform::{
            components::{GlobalTransform, Transform},
            TransformBundle, TransformPlugin,
        },
        utils::HashSet,
        window::WindowPlugin,
    };
    use bevy_rapier2d::{
        geometry::{Collider, Sensor},
        plugin::{NoUserData, RapierContext, RapierPhysicsPlugin},
    };
    use proptest::{prelude::*, strategy::ValueTree};
    use proptest::{strategy::Strategy, test_runner::TestRunner};
    use rand::seq::SliceRandom;

    use crate::{
        inventory::{Inventory, Storage},
        structure_components::{
            inserter::{
                find_belt_pickups_for_entity, find_inventory_pickups_for_entity, find_pickups,
                Dropoff, Inserter, InserterTargetItem, Pickup,
            },
            transport_belt::TransportBelt,
        },
        types::Item,
        util::Inventories,
    };
    // Strategy to generate random items
    fn arb_item() -> impl Strategy<Value = Item> {
        "Item [a-zA-Z0-9]{1,10}".prop_map(Item::new)
    }

    // Strategy to generate random inventory with items
    fn arb_inventory() -> impl Strategy<Value = Inventory> {
        prop::collection::vec(arb_item(), 1..10).prop_map(|items| {
            let mut inventory = Inventory::new(10);
            for item in items {
                inventory.add_item(&item, 1);
            }
            inventory
        })
    }

    // Strategy to generate random InserterTargetItem
    fn arb_target_item(items: &HashSet<Item>) -> impl Strategy<Value = InserterTargetItem> {
        let item_vec = items.iter().cloned().collect::<Vec<_>>();

        prop_oneof![
            Just(InserterTargetItem::Any),
            prop::collection::vec(prop::sample::select(item_vec.clone()), 1..=item_vec.len())
                .prop_map(InserterTargetItem::Filter)
        ]
    }

    proptest! {
        #[test]
        fn test_find_inventory_pickups(
            inventory in arb_inventory(),
        ) {
            let mut app = App::new();

            let inventory_entity = app.world.spawn((inventory.clone(), Storage)).id();

            let mut items = HashSet::<Item>::new();
            items.extend(inventory.slots.iter().flatten().map(|stack| stack.item.clone()));

            let mut test_runner = TestRunner::default();
            let target_item = arb_target_item(&items).new_tree(&mut test_runner).unwrap().current();
            let target_item_1 = target_item.clone();

            app.add_systems(Update, move |inventories: Inventories| {
                let pickups = find_inventory_pickups_for_entity(inventory_entity, &inventories, &target_item_1).collect::<Vec<_>>();

                match &target_item_1 {
                    InserterTargetItem::Any => {
                        assert_eq!(
                            pickups.len(),
                            inventory.slots.iter().flatten().count(),
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            inventory.slots.iter().flatten().count(),
                            pickups.len(),
                        );
                    }
                    InserterTargetItem::Filter(filter) => {
                        let expected_pickups = inventory.slots.iter().flatten().filter(|stack| filter.contains(&stack.item)).count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Filter({:?})'. Expected: {}, Found: {}",
                            filter,
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                }
            });

            // Test empty inventory
            let empty_inventory_entity = app.world.spawn((Inventory::new(10), Storage)).id();
            app.add_systems(Update, move |inventories: Inventories| {
                let pickups = find_inventory_pickups_for_entity(empty_inventory_entity, &inventories, &target_item).collect::<Vec<_>>();
                assert!(pickups.is_empty(), "There should be no pickups for an empty inventory.");
            });

            // Test target item not in inventory
            let nonexistent_item = Item::new("Nonexistent");
            let nonexistent_target = InserterTargetItem::Filter(vec![nonexistent_item]);
            app.add_systems(Update, move |inventories: Inventories| {
                let pickups = find_inventory_pickups_for_entity(inventory_entity, &inventories, &nonexistent_target).collect::<Vec<_>>();
                assert!(pickups.is_empty(), "There should be no pickups for a nonexistent target item.");
            });

            // Run the app update to execute the systems
            app.update();
        }
    }

    prop_compose! {
        fn arb_belt_slots()
        (input in prop::collection::vec(arb_item(), 1..=3)) -> Vec<Option<Item>>
        {
            let mut indices = [0, 1, 2];
            indices.shuffle(&mut rand::thread_rng());
            let mut slots = vec![None, None, None];
            for (i, item) in input.into_iter().enumerate() {
                slots[indices[i]] = Some(item);
            }
            slots
        }
    }

    proptest! {
        #[test]
        fn test_find_belt_pickups(
            items_in_slots in arb_belt_slots(),
        ) {
            let mut app = App::new();

            let dropoff_entity = app.world.spawn_empty().id();
            let mut belt = TransportBelt::new(dropoff_entity);
            for (i, item) in items_in_slots.iter().enumerate() {
                *belt.slot_mut(i).unwrap() = item.clone();

            }
            let belt_entity = app.world.spawn(belt).id();

            // Collect all items in the belt slots
            let items = items_in_slots.into_iter().flatten().collect::<HashSet<_>>();

            let mut test_runner = TestRunner::default();
            let target_item = arb_target_item(&items).new_tree(&mut test_runner).unwrap().current();
            let target_item_1 = target_item.clone();

            app.add_systems(Update, move |belts_query: Query<&TransportBelt>| {
                let pickups = find_belt_pickups_for_entity(belt_entity, &belts_query, &target_item_1).collect::<Vec<_>>();

                match &target_item_1 {
                    InserterTargetItem::Any => {
                        let expected_pickups = belts_query.get(belt_entity).unwrap().slot(1).unwrap().iter().count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                    InserterTargetItem::Filter(filter) => {
                        let expected_pickups = belts_query.get(belt_entity).unwrap().slot(1).unwrap().iter().filter(|item| filter.contains(item)).count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Filter({:?})'. Expected: {}, Found: {}",
                            filter,
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                }
            });

            let empty_belt_entity = app.world.spawn(TransportBelt::new(dropoff_entity)).id();

            app.add_systems(Update, move |belts_query: Query<&TransportBelt>| {
                let pickups = find_belt_pickups_for_entity(empty_belt_entity, &belts_query, &target_item).collect::<Vec<_>>();

                assert_eq!(pickups.len(), 0, "There should be no pickups for an empty belt.");
            });

            app.add_systems(Update, move |belts_query: Query<&TransportBelt>| {
                // Create a nonexistent target item
                let nonexistent_item = Item::new("Nonexistent");

                let pickups = find_belt_pickups_for_entity(belt_entity, &belts_query, &InserterTargetItem::Filter(vec![nonexistent_item])).collect::<Vec<_>>();

                assert_eq!(pickups.len(), 0, "There should be no pickups for a nonexistent target item.");
            });

            // Run the app update to execute the system
            app.update();
        }
    }

    proptest! {

        #[test]
        fn test_find_pickups(
            inventory in arb_inventory(),
            items_in_slots in arb_belt_slots(),
        ) {
            // Create a Bevy App with necessary plugins
            let mut app = App::new();
            app.add_plugins((
                HeadlessRenderPlugin,
                TransformPlugin,
                TimePlugin,
                RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0)
            ));

            let pickup_transform = Transform::from_xyz(-1.0, 0.0, 0.0);
            let dropoff_transform = Transform::from_xyz(1.0, 0.0, 0.0);

            let inserter_entity = {
                let inserter_transform = Transform::from_xyz(0.0, 0.0, 0.0);
                let pickup = app
                .world
                .spawn((
                    TransformBundle::from(pickup_transform),
                    Pickup,
                ))
                .id();
                let dropoff = app
                    .world
                    .spawn((
                        TransformBundle::from(dropoff_transform),
                        Dropoff,
                    ))
                    .id();

                app
                    .world
                    .spawn((
                        TransformBundle::from(inserter_transform),
                        Inserter::new(1.0, 10, dropoff, pickup),
                    )).push_children(&[pickup, dropoff])
                    .id()
            };

            let belt_entity = {
                let dropoff_entity = app.world.spawn_empty().id();
                let mut belt = TransportBelt::new(dropoff_entity);
                for (i, item) in items_in_slots.iter().enumerate() {
                    *belt.slot_mut(i).unwrap() = item.clone();

                }
                app.world.spawn((
                    belt,
                    Collider::ball(0.5),
                    TransformBundle::from(pickup_transform),
                )).id()
            };

            let inventory_entity = app.world.spawn((
                inventory.clone(),
                Storage,
                Sensor,
                Collider::ball(0.5),
                TransformBundle::from(pickup_transform),
            )).id();

            app.update();

            app.add_systems(Update, move |
                inventories: Inventories,
                belts_query: Query<&TransportBelt>,
                rapier_context: Res<RapierContext>,
                collider_query: Query<(&Collider, &GlobalTransform)>,
                inserter_query: Query<&Inserter>,
                | {
                let mut items = HashSet::<Item>::new();
                items.extend(inventory.slots.iter().flatten().map(|stack| stack.item.clone()));

                let mut test_runner = TestRunner::default();
                let target_item = arb_target_item(&items).new_tree(&mut test_runner).unwrap().current();
                let target_item_1 = target_item.clone();

                let inserter = inserter_query.get(inserter_entity).unwrap();

                let pickups = find_pickups(
                    inserter,
                    &inventories,
                    &belts_query,
                    &rapier_context,
                    &collider_query,
                    &target_item_1,
                );

                match &target_item_1 {
                    InserterTargetItem::Any => {
                        let expected_pickups = inventory.slots.iter().flatten().count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                    InserterTargetItem::Filter(filter) => {
                        let expected_pickups = inventory.slots.iter().flatten().filter(|stack| filter.contains(&stack.item)).count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Filter({:?})'. Expected: {}, Found: {}",
                            filter,
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                }
            });

            // Update the app to synchronize transforms and physics
            app.update();


        }
    }

    struct HeadlessRenderPlugin;

    impl Plugin for HeadlessRenderPlugin {
        fn build(&self, app: &mut App) {
            app.add_plugins((
                WindowPlugin::default(),
                AssetPlugin::default(),
                ScenePlugin,
                RenderPlugin {
                    render_creation: RenderCreation::Automatic(WgpuSettings {
                        backends: None,
                        ..Default::default()
                    }),
                },
                ImagePlugin::default(),
            ));
        }
    }
}
