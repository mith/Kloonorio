use bevy::{math::Vec3Swizzles, prelude::*};

use crate::{
    inventory::{Inventory, InventoryParams, InventoryType, Stack, MAX_STACK_SIZE},
    item::Item,
    tile_occupants::TileOccupants,
    types::{AppState, Powered, Working},
};

use super::transport_belt::{TransportBelt, TransportBeltSet};

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct InserterSet;

pub struct InserterPlugin;

impl Plugin for InserterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            (
                inserter_planner,
                apply_deferred.in_set(InserterFlush),
                inserter_tick,
            )
                .chain()
                .in_set(InserterSet)
                .after(TransportBeltSet)
                .run_if(in_state(AppState::Running)),
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
    pickup_tile: Entity,
    dropoff_tile: Entity,
    current_action: Option<InserterAction>,
    speed: f32,
}

impl Inserter {
    pub fn new(
        speed: f32,
        capacity: u32,
        pickup_location_entity: Entity,
        dropoff_location_entity: Entity,
    ) -> Self {
        Inserter {
            holding: None,
            capacity,
            arm_position: 0.,
            target_arm_position: 0.,
            pickup_tile: pickup_location_entity,
            dropoff_tile: dropoff_location_entity,
            current_action: None,
            speed,
        }
    }

    pub fn arm_position(&self) -> f32 {
        self.arm_position
    }

    pub fn holding(&self) -> Option<&Stack> {
        self.holding.as_ref()
    }
}

#[derive(Component, Debug, Reflect)]
pub struct InserterHand(pub Entity);

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
enum InserterTargetType {
    Belt(Entity),
    Inventory(Entity),
    ItemOnGround(Entity),
}

impl InserterTargetType {
    fn entity(&self) -> Entity {
        match self {
            InserterTargetType::Belt(entity) => *entity,
            InserterTargetType::Inventory(entity) => *entity,
            InserterTargetType::ItemOnGround(entity) => *entity,
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
enum PickupTarget {
    Any,
    Filter(Vec<Item>),
}

impl PickupTarget {
    fn contains(&self, item: &Item) -> bool {
        match self {
            PickupTarget::Any => true,
            PickupTarget::Filter(filter) => filter.contains(item),
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct DropoffRequest {
    target_type: InserterTargetType,
    target_item: PickupTarget,
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
    inventories: &'a InventoryParams,
    target_item: &'a PickupTarget,
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
                            if target_item.contains(&stack.item) {
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
    target_item: &'a PickupTarget,
) -> impl Iterator<Item = AvailablePickup> + 'a {
    belts_query
        .get(entity)
        .ok()
        .into_iter()
        .flat_map(move |belt| {
            belt.slot(1).unwrap().as_ref().and_then(|item| {
                let is_target_item = match target_item {
                    PickupTarget::Any => true,
                    PickupTarget::Filter(ref filter) => filter.contains(item),
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
    inventories: &InventoryParams,
    belts_query: &Query<&TransportBelt>,
    belt_occupants_query: &Query<&TileOccupants>,
    target_item: &PickupTarget,
) -> Vec<AvailablePickup> {
    belt_occupants_query
        .get(inserter.pickup_tile)
        .map(|o| o.iter())
        .into_iter()
        .flatten()
        .flat_map(move |&entity| {
            let inventory_pickups =
                find_inventory_pickups_for_entity(entity, inventories, target_item);
            let belt_pickups = find_belt_pickups_for_entity(entity, belts_query, target_item);
            inventory_pickups.chain(belt_pickups)
        })
        .collect()
}

fn find_inventory_dropoffs_for_entity<'a>(
    entity: Entity,
    inventories: &'a InventoryParams<'_, '_>,
    target_item: Option<&'a Item>,
) -> impl Iterator<Item = DropoffRequest> + 'a {
    [
        InventoryType::Fuel,
        InventoryType::Source,
        InventoryType::Storage,
    ]
    .iter()
    .filter_map(move |&inventory_type| inventories.get_inventory_component(entity, inventory_type))
    .filter(move |inventory| target_item.map_or(true, |item| inventory.can_add_item(item)))
    .flat_map(move |inventory| {
        // Create dropoffrequests for all partial stacks
        inventory
            .slots
            .iter()
            .filter_map(move |slot| {
                slot.as_ref().and_then(|stack| {
                    if (target_item.is_none() || target_item == Some(&stack.item))
                        && stack.amount < MAX_STACK_SIZE
                    {
                        Some(DropoffRequest {
                            target_type: InserterTargetType::Inventory(entity),
                            target_item: PickupTarget::Filter(vec![stack.item.clone()]),
                        })
                    } else {
                        None
                    }
                })
            })
            .chain(
                // Then create a dropoffrequest for an empty slot if there is one
                if inventory.slots.iter().any(|slot| slot.is_none()) {
                    Some(DropoffRequest {
                        target_type: InserterTargetType::Inventory(entity),
                        target_item: PickupTarget::Any,
                    })
                } else {
                    None
                },
            )
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
                    target_item: PickupTarget::Any,
                })
            } else {
                None
            }
        })
}

fn find_dropoffs(
    inserter: &Inserter,
    inventories: &InventoryParams,
    belts_query: &Query<&TransportBelt>,
    tile_occupants_query: &Query<&TileOccupants>,
    target_item: Option<&Item>,
) -> Vec<DropoffRequest> {
    tile_occupants_query
        .get(inserter.dropoff_tile)
        .map(|o| o.iter())
        .into_iter()
        .flatten()
        .flat_map(move |&entity| {
            let inventory_dropoffs =
                find_inventory_dropoffs_for_entity(entity, inventories, target_item);
            let belt_dropoffs = find_belt_dropoffs_for_entity(entity, belts_query);

            inventory_dropoffs.chain(belt_dropoffs)
        })
        .collect()
}

fn plan_inserter_action(
    inserter: &Inserter,
    inventories: &InventoryParams,
    belts_query: &Query<&TransportBelt>,
    tile_occupants_query: &Query<&TileOccupants>,
) -> Option<InserterAction> {
    let dropoffs = find_dropoffs(
        inserter,
        inventories,
        belts_query,
        tile_occupants_query,
        inserter.holding.as_ref().map(|stack| &stack.item),
    );
    if let Some(holding) = inserter.holding.as_ref() {
        dropoffs
            .iter()
            .find(|d| match d.target_item {
                PickupTarget::Any => true,
                PickupTarget::Filter(ref filter) => filter.contains(&holding.item),
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
                tile_occupants_query,
                &dropoff.target_item,
            )
            .iter()
            .find(|pickup| match dropoff.target_item {
                PickupTarget::Any => true,
                PickupTarget::Filter(ref filter) => filter.contains(&pickup.target_item),
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
    tile_occupants_query: &'a Query<'w, 's, &TileOccupants>,
    action: &'a InserterAction,
) -> bool {
    let holding_valid = inserter
        .holding
        .as_ref()
        .map(|stack| action.item == stack.item)
        .unwrap_or(true);

    if !holding_valid {
        debug!("Holding item does not match action item");
        return false;
    }

    let dropoff_tile_occupants = tile_occupants_query.get(inserter.dropoff_tile).unwrap();

    if !dropoff_tile_occupants.contains(&action.dropoff.entity()) {
        debug!("Dropoff entity is not on the dropoff tile");
        return false;
    }

    let dropoff_valid = match action.dropoff {
        InserterTargetType::Belt(entity) => belts_query
            .get(entity)
            .ok()
            .map(|belt| belt.can_add(1))
            .unwrap_or(false),
        InserterTargetType::Inventory(entity) => {
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
        debug!("Dropoff entity is not valid");
        return false;
    }

    if inserter.holding.is_some() {
        return true;
    }

    if let Some(pickup_target) = action.pickup.as_ref() {
        let pickup_tile_occupants = tile_occupants_query.get(inserter.pickup_tile).unwrap();
        if !pickup_tile_occupants.contains(&pickup_target.entity()) {
            debug!("Pickup entity is not on the pickup tile");
            return false;
        }

        let pickup_valid = action.pickup.as_ref().map_or(true, |pickup| {
            match pickup {
                InserterTargetType::Belt(entity) => belts_query
                    .get(*entity)
                    .ok()
                    .map(|belt| belt.slot(1).unwrap().is_some())
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
        if !pickup_valid {
            debug!("Pickup entity is not valid");
            return false;
        }
    }

    true
}

fn inserter_planner(
    mut commands: Commands,
    mut inserter_query: Query<(Entity, &mut Inserter), With<Powered>>,
    tile_occupants_query: Query<&TileOccupants>,
    mut inventories_set: ParamSet<(InventoryParams, Query<&Inventory>)>,
    belts_query: Query<&TransportBelt>,
) {
    for (inserter_entity, mut inserter) in &mut inserter_query {
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
                            &tile_occupants_query,
                            current_action,
                        )
                    });

            if new_action_needed {
                trace!("Planning new action");
                let new_action = plan_inserter_action(
                    &inserter,
                    &inventories_set.p0(),
                    &belts_query,
                    &tile_occupants_query,
                );
                inserter.target_arm_position = if inserter.holding.is_some() {
                    1.0
                } else {
                    -1.0
                };
                debug!(target_arm_position = ?inserter.target_arm_position);
                if new_action.is_some() {
                    commands.entity(inserter_entity).insert(Working);
                    debug!(action=?new_action, "New action planned");
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
                    inserter.target_arm_position = 1.0;
                }
            }
        }
    }
}

fn inserter_with_offset(inserter_transform: &GlobalTransform, direction: Vec3) -> Vec2 {
    inserter_transform
        .mul_transform(Transform::from_translation(direction))
        .translation()
        .xy()
}

pub const INSERTER_PICKUP_OFFSET: Vec3 = Vec3::new(-1., 0., 0.);
pub const INSERTER_DROPOFF_OFFSET: Vec3 = Vec3::new(1., 0., 0.);

pub fn inserter_dropoff_location(inserter_transform: &GlobalTransform) -> Vec2 {
    inserter_with_offset(inserter_transform, INSERTER_DROPOFF_OFFSET)
}

pub fn inserter_pickup_location(inserter_transform: &GlobalTransform) -> Vec2 {
    inserter_with_offset(inserter_transform, INSERTER_PICKUP_OFFSET)
}

#[cfg(test)]
mod test {
    use bevy::{
        app::{App, Update},
        ecs::system::Query,
        utils::HashSet,
    };
    use proptest::{prelude::*, strategy::ValueTree};
    use proptest::{strategy::Strategy, test_runner::TestRunner};
    use rand::seq::SliceRandom;

    use crate::{
        inventory::{Inventory, InventoryParams, Stack, Storage, MAX_STACK_SIZE},
        item::Item,
        structure_components::{
            inserter::{
                find_belt_pickups_for_entity, find_inventory_dropoffs_for_entity,
                find_inventory_pickups_for_entity, find_pickups, inserter_planner, Inserter,
                PickupTarget,
            },
            transport_belt::TransportBelt,
        },
        tile_occupants::TileOccupants,
        types::Powered,
    };
    // Strategy to generate random items
    fn arb_item() -> impl Strategy<Value = Item> {
        "[a-zA-Z0-9]{1,10}".prop_map(Item::new)
    }

    fn arb_items() -> impl Strategy<Value = HashSet<Item>> {
        prop::collection::vec(arb_item(), 1..10).prop_map(|items| items.iter().cloned().collect())
    }

    fn pick_arb_item(items: &HashSet<Item>) -> impl Strategy<Value = Item> {
        prop::sample::select(items.iter().cloned().collect::<Vec<_>>())
    }

    fn arb_item_not_in(items: &HashSet<Item>) -> impl Strategy<Value = Item> {
        let items = items.clone();
        arb_item().prop_filter("Item not in set", move |item| !items.contains(item))
    }

    // Strategy to generate random inventory with items
    fn arb_inventory() -> impl Strategy<Value = Inventory> {
        let empty_inventory = Just(Inventory::new(10));
        prop_oneof![
            empty_inventory,
            arb_partial_inventory(),
            arb_all_partial_stacks_inventory(),
            arb_full_inventory()
        ]
    }

    fn arb_partial_inventory() -> impl Strategy<Value = Inventory> {
        prop::collection::vec(arb_item(), 1..10).prop_map(|items| {
            let mut inventory = Inventory::new(10);
            for item in items {
                inventory.add_item(&item, 1);
            }
            inventory
        })
    }

    fn arb_full_inventory() -> impl Strategy<Value = Inventory> {
        // Generate an inventory with all slots containing a full stack of a random item
        prop::collection::vec(arb_item(), 10).prop_map(|items| {
            let mut inventory = Inventory::new(10);
            for item in items {
                inventory.add_item(&item, MAX_STACK_SIZE);
            }
            inventory
        })
    }

    fn arb_all_partial_stacks_inventory() -> impl Strategy<Value = Inventory> {
        let mut inventory = Inventory::new(10);
        for (i, slot) in inventory.slots.iter_mut().enumerate() {
            *slot = Some(Stack::new(
                Item::new(format!("Item {}", i)),
                MAX_STACK_SIZE / 2,
            ));
        }
        Just(inventory)
    }

    // Strategy to generate a random pickup target from a set of items
    fn arb_pickup_target(items: &HashSet<Item>) -> impl Strategy<Value = PickupTarget> {
        let item_vec = items.iter().cloned().collect::<Vec<_>>();

        prop_oneof![
            Just(PickupTarget::Any),
            prop::collection::vec(prop::sample::select(item_vec), 1..10)
                .prop_map(PickupTarget::Filter)
        ]
    }

    fn arb_filtered_inventory(items: &HashSet<Item>) -> impl Strategy<Value = Inventory> {
        let item_vec = items.iter().cloned().collect::<Vec<_>>();

        prop::collection::vec(prop::sample::select(item_vec), 1..10).prop_map(|items| {
            let inventory = Inventory::new_with_filter(10, items.iter().cloned().collect());
            inventory
        })
    }

    proptest! {
        #[test]
        fn test_find_inventory_pickups_for_entity(
            inventory in arb_partial_inventory(),
        ) {
            let mut app = App::new();

            let inventory_entity = app.world.spawn((inventory.clone(), Storage)).id();

            let mut items = HashSet::<Item>::new();
            items.extend(inventory.slots.iter().flatten().map(|stack| stack.item.clone()));

            let mut test_runner = TestRunner::default();
            let pickup_target = arb_pickup_target(&items).new_tree(&mut test_runner).unwrap().current();
            let pickup_target_1 = pickup_target.clone();

            app.add_systems(Update, move |inventories: InventoryParams| {
                let pickups = find_inventory_pickups_for_entity(inventory_entity, &inventories, &pickup_target_1).collect::<Vec<_>>();

                match &pickup_target_1 {
                    PickupTarget::Any => {
                        assert_eq!(
                            pickups.len(),
                            inventory.slots.iter().flatten().count(),
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            inventory.slots.iter().flatten().count(),
                            pickups.len(),
                        );
                    }
                    PickupTarget::Filter(filter) => {
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
            app.add_systems(Update, move |inventories: InventoryParams| {
                let pickups = find_inventory_pickups_for_entity(empty_inventory_entity, &inventories, &pickup_target).collect::<Vec<_>>();
                assert!(pickups.is_empty(), "There should be no pickups for an empty inventory.");
            });

            // Test target item not in inventory
            let nonexistent_item = Item::new("Nonexistent");
            let nonexistent_target = PickupTarget::Filter(vec![nonexistent_item]);
            app.add_systems(Update, move |inventories: InventoryParams| {
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
        fn test_find_belt_pickups_for_entity(
            items_in_slots in arb_belt_slots(),
        ) {
            let mut app = App::new();

            let mut belt = TransportBelt::default();
            for (i, item) in items_in_slots.iter().enumerate() {
                *belt.slot_mut(i).unwrap() = item.clone();

            }
            let belt_entity = app.world.spawn(belt).id();

            // Collect all items in the belt slots
            let items = items_in_slots.into_iter().flatten().collect::<HashSet<_>>();

            let mut test_runner = TestRunner::default();
            let pickup_target = arb_pickup_target(&items).new_tree(&mut test_runner).unwrap().current();
            let pickup_target_1 = pickup_target.clone();

            app.add_systems(Update, move |belts_query: Query<&TransportBelt>| {
                let pickups = find_belt_pickups_for_entity(belt_entity, &belts_query, &pickup_target_1).collect::<Vec<_>>();

                match &pickup_target_1 {
                    PickupTarget::Any => {
                        let expected_pickups = belts_query.get(belt_entity).unwrap().slot(1).unwrap().iter().count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                    PickupTarget::Filter(filter) => {
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

            let empty_belt_entity = app.world.spawn(TransportBelt::default()).id();

            app.add_systems(Update, move |belts_query: Query<&TransportBelt>| {
                let pickups = find_belt_pickups_for_entity(empty_belt_entity, &belts_query, &pickup_target).collect::<Vec<_>>();

                assert_eq!(pickups.len(), 0, "There should be no pickups for an empty belt.");
            });

            app.add_systems(Update, move |belts_query: Query<&TransportBelt>| {
                // Create a nonexistent target item
                let nonexistent_item = Item::new("Nonexistent");

                let pickups = find_belt_pickups_for_entity(belt_entity, &belts_query, &PickupTarget::Filter(vec![nonexistent_item])).collect::<Vec<_>>();

                assert_eq!(pickups.len(), 0, "There should be no pickups for a nonexistent target item.");
            });

            // Run the app update to execute the system
            app.update();
        }
    }

    proptest! {

        #[test]
        fn test_find_pickups_from_inventory(
            inventory in arb_partial_inventory(),
        ) {
            // Create a Bevy App with necessary plugins
            let mut app = App::new();
            let pickup_inventory_entity = app.world.spawn((
                inventory.clone(),
                Storage,
            )).id();


            let pickup_tile = app.world.spawn(TileOccupants::new([pickup_inventory_entity].into())).id();
            let dropoff_tile = app.world.spawn(TileOccupants::new([].into())).id();
            let inserter_entity =
                app
                    .world
                    .spawn(Inserter::new(1.0, 10, pickup_tile, dropoff_tile))
                    .id();

            let mut items = HashSet::<Item>::new();
            items.extend(inventory.slots.iter().flatten().map(|stack| stack.item.clone()));

            let mut test_runner = TestRunner::default();
            let pickup_target = arb_pickup_target(&items).new_tree(&mut test_runner).unwrap().current();
            let pickup_target_1 = pickup_target.clone();

            app.add_systems(Update, move |
                inventories: InventoryParams,
                belts_query: Query<&TransportBelt>,
                tile_occupants_query: Query<&TileOccupants>,
                inserter_query: Query<&Inserter>,
                | {
                let pickups = find_pickups(
                    inserter_query.get(inserter_entity).unwrap(),
                    &inventories,
                    &belts_query,
                    &tile_occupants_query,
                    &pickup_target_1,
                );

                match &pickup_target_1 {
                    PickupTarget::Any => {
                        let expected_pickups = inventory.slots.iter().flatten().count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                    PickupTarget::Filter(filter) => {
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

    proptest! {
        #[test]
        fn test_find_pickups_from_belt(
            items_in_slots in arb_belt_slots(),
        ) {
            // Create a Bevy App with necessary plugins
            let mut app = App::new();

            let mut belt = TransportBelt::default();
            for (i, item) in items_in_slots.iter().enumerate() {
                *belt.slot_mut(i).unwrap() = item.clone();

            }
            let belt_entity = app.world.spawn(belt).id();

            let pickup_tile = app.world.spawn(TileOccupants::new([belt_entity].into())).id();
            let dropoff_tile = app.world.spawn(TileOccupants::new([].into())).id();
            let inserter_entity =
                app
                    .world
                    .spawn(Inserter::new(1.0, 10, pickup_tile, dropoff_tile))
                    .id();

            let items = items_in_slots.into_iter().flatten().collect::<HashSet<_>>();

            let mut test_runner = TestRunner::default();
            let pickup_target = arb_pickup_target(&items).new_tree(&mut test_runner).unwrap().current();
            let pickup_target_1 = pickup_target.clone();

            app.add_systems(Update, move |
                inventories: InventoryParams,
                belts_query: Query<&TransportBelt>,
                tile_occupants_query: Query<&TileOccupants>,
                inserter_query: Query<&Inserter>,
                | {
                let pickups = find_pickups(
                    inserter_query.get(inserter_entity).unwrap(),
                    &inventories,
                    &belts_query,
                    &tile_occupants_query,
                    &pickup_target_1,
                );

                match &pickup_target_1 {
                    PickupTarget::Any => {
                        let expected_pickups = belts_query.get(belt_entity).unwrap().slot(1).unwrap().iter().count();
                        assert_eq!(
                            pickups.len(),
                            expected_pickups,
                            "Mismatch in pickups count for target 'Any'. Expected: {}, Found: {}",
                            expected_pickups,
                            pickups.len(),
                        );
                    }
                    PickupTarget::Filter(filter) => {
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

            // Update the app to synchronize transforms and physics
            app.update();
        }
    }

    proptest! {
        #[test]
        fn test_find_inventory_dropoffs_for_entity(
            inventory in arb_inventory(),
        ) {
            // Create a Bevy App with necessary plugins
            let mut app = App::new();

            let dropoff_inventory_entity = app.world.spawn((
                inventory.clone(),
                Storage,
            )).id();

            let mut items = HashSet::<Item>::new();
            items.extend(inventory.slots.iter().flatten().map(|stack| stack.item.clone()));

            let mut test_runner = TestRunner::default();
            let target_item = arb_item().new_tree(&mut test_runner).unwrap().current();
            let target_item_1 = target_item.clone();

            app.add_systems(Update, move |
                inventories: InventoryParams,
                | {
                let target_item_1 = Some(&target_item_1);
                let dropoffs =
                    find_inventory_dropoffs_for_entity(
                        dropoff_inventory_entity,
                        &inventories,
                        target_item_1
                    ).collect::<Vec<_>>();

                match target_item_1.as_ref() {
                    None => {
                        let expected_dropoffs = if inventory.slots.iter().any(|slot| slot.is_none()) {
                            1
                        } else {
                            0
                        };
                        assert_eq!(
                            dropoffs.len(),
                            expected_dropoffs,
                            "Mismatch in dropoffs count for target 'Any'. Expected: {}, Found: {}",
                            expected_dropoffs,
                            dropoffs.len(),
                        );
                    }
                    Some(&filter) => {
                        let num_partial_item_slots = inventory.slots.iter().flatten().filter(|stack| stack.item == *filter && stack.amount < MAX_STACK_SIZE).count().min(1);
                        let num_empty_slots = inventory.slots.iter().filter(|slot| slot.is_none()).count().min(1);
                        let expected_dropoffs = (num_partial_item_slots + num_empty_slots).min(1);
                        assert_eq!(
                            dropoffs.len(),
                            expected_dropoffs,
                            "Mismatch in dropoffs count for target 'Filter({:?})'. Expected: {}, Found: {}",
                            filter,
                            expected_dropoffs,
                            dropoffs.len(),
                        );
                    }
                }
            });

            // Update the app to synchronize transforms and physics
            app.update();
        }
    }

    proptest! {
        #[test]
        fn test_find_inventory_dropoffs_for_entity_filtered_inventory(
            items in arb_items(),
        ) {
            let mut app = App::new();

            let mut test_runner = TestRunner::default();
            let filtered_inventory = arb_filtered_inventory(&items).new_tree(&mut test_runner).unwrap().current();
            let allowed_items = filtered_inventory.allowed_items.into_set();

            let allowed_target_item = pick_arb_item(&allowed_items).new_tree(&mut test_runner).unwrap().current();
            let disallowed_target_item = arb_item_not_in(&allowed_items).new_tree(&mut test_runner).unwrap().current();

            let dropoff_inventory_entity = app.world.spawn((
                filtered_inventory.clone(),
                Storage,
            )).id();

            app.add_systems(Update, move |
                inventories: InventoryParams,
                | {
                let allowed_dropoffs =
                    find_inventory_dropoffs_for_entity(
                        dropoff_inventory_entity,
                        &inventories,
                        Some(&allowed_target_item)
                    ).collect::<Vec<_>>();

                assert_eq!(
                    allowed_dropoffs.len(),
                    1,
                    "Mismatch in allowed dropoffs count for target '{:?}'. Expected: {}, Found: {}",
                    allowed_target_item,
                    1,
                    allowed_dropoffs.len(),
                );

                let disallowed_dropoffs =
                    find_inventory_dropoffs_for_entity(
                        dropoff_inventory_entity,
                        &inventories,
                        Some(&disallowed_target_item)
                    ).collect::<Vec<_>>();

                assert_eq!(
                    disallowed_dropoffs.len(),
                    0,
                    "Mismatch in disallowed dropoffs count for target '{:?}' ({} allowed). Expected: {}, Found: {}",
                    disallowed_target_item,
                    allowed_items.iter().map(|item| format!("{:?}", item)).collect::<Vec<_>>().join(", "),
                    0,
                    disallowed_dropoffs.len(),
                );
            });

            app.update();
        }
    }

    proptest! {
        #[test]
        fn test_inserter_planner_empty_dropoff(
            pickup_inventory in arb_inventory(),
        ) {
            let mut app = App::new();

            let pickup_inventory_entity = app.world.spawn((
                pickup_inventory.clone(),
                Storage,
            )).id();

            let dropoff_inventory_entity = app.world.spawn((
                Inventory::new(10),
                Storage,
            )).id();

            let pickup_tile_entity = app.world.spawn(TileOccupants::new([pickup_inventory_entity].into())).id();
            let dropoff_tile_entity = app.world.spawn(TileOccupants::new([dropoff_inventory_entity].into())).id();

            let inserter_entity =
                app
                    .world
                    .spawn((Inserter::new(1.0, 10, pickup_tile_entity, dropoff_tile_entity), Powered))
                    .id();

            let any_pickup = pickup_inventory.slots.iter().any(|slot| slot.is_some());

            app.add_systems(Update, inserter_planner);

            app.update();

            if any_pickup {
                let inserter = app.world.get::<Inserter>(inserter_entity).unwrap();
                assert!(inserter.current_action.is_some(), "Inserter should have a current action");
            } else {
                let inserter = app.world.get::<Inserter>(inserter_entity).unwrap();
                assert!(inserter.current_action.is_none(), "Inserter should not have a current action");
            }
        }
    }

    proptest! {
        #[test]
        fn test_inserter_planner_full_dropoff(
            pickup_inventory in arb_inventory(),
            dropoff_inventory in arb_full_inventory(),
        ) {
            let mut app = App::new();

            let pickup_inventory_entity = app.world.spawn((
                pickup_inventory.clone(),
                Storage,
            )).id();

            let dropoff_inventory_entity = app.world.spawn((
                dropoff_inventory.clone(),
                Storage,
            )).id();

            let pickup_tile_entity = app.world.spawn(TileOccupants::new([pickup_inventory_entity].into())).id();
            let dropoff_tile_entity = app.world.spawn(TileOccupants::new([dropoff_inventory_entity].into())).id();

            let inserter_entity =
                app
                    .world
                    .spawn((Inserter::new(1.0, 10, pickup_tile_entity, dropoff_tile_entity), Powered))
                    .id();


            app.add_systems(Update, inserter_planner);

            app.update();

            let inserter = app.world.get::<Inserter>(inserter_entity).unwrap();
            assert!(inserter.current_action.is_none(), "Inserter should not have a current action");
        }

    }

    proptest! {
        #[test]
        fn test_inserter_planner_partial_dropoff(
            pickup_inventory in arb_inventory(),
            dropoff_inventory in arb_all_partial_stacks_inventory(),
        ) {
            let mut app = App::new();

            let pickup_inventory_entity = app.world.spawn((
                pickup_inventory.clone(),
                Storage,
            )).id();

            let dropoff_inventory_entity = app.world.spawn((
                dropoff_inventory.clone(),
                Storage,
            )).id();

            let pickup_tile_entity = app.world.spawn(TileOccupants::new([pickup_inventory_entity].into())).id();
            let dropoff_tile_entity = app.world.spawn(TileOccupants::new([dropoff_inventory_entity].into())).id();

            let inserter_entity =
                app
                    .world
                    .spawn((Inserter::new(1.0, 10, pickup_tile_entity, dropoff_tile_entity), Powered))
                    .id();

            let any_matching_items = {
                let pickup_set = pickup_inventory.slots.iter().flatten().map(|stack| stack.item.clone()).collect::<HashSet<_>>();
                let dropoff_set = dropoff_inventory.slots.iter().flatten().map(|stack| stack.item.clone()).collect::<HashSet<_>>();
                pickup_set.intersection(&dropoff_set).next().is_some()
            };

            app.add_systems(Update, inserter_planner);
            app.update();

            if any_matching_items {
                let inserter = app.world.get::<Inserter>(inserter_entity).unwrap();
                assert!(inserter.current_action.is_some(), "Inserter should have a current action");
            } else {
                let inserter = app.world.get::<Inserter>(inserter_entity).unwrap();
                assert!(inserter.current_action.is_none(), "Inserter should not have a current action");
            }
        }
    }
}
