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
                (inserter_planner, inserter_tick).chain(),
                burner_inserter_tick,
            ),
        )
        .register_type::<Inserter>();
    }
}

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

fn find_belt_pickups_for_entity(
    entity: Entity,
    belts_query: &Query<&TransportBelt>,
    target_item: &InserterTargetItem,
) -> Vec<AvailablePickup> {
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
        .collect()
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
        .map(|(_c, t)| {
            let pickup_entities =
                find_entities_on_position(rapier_context, t.translation().xy(), None);
            pickup_entities
        })
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

fn find_inventory_dropoffs_for_entity<'w, 's, 'a>(
    entity: Entity,
    inventories: &'a Inventories<'w, 's>,
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

fn find_belt_dropoffs_for_entity<'w, 's, 'a>(
    entity: Entity,
    belts_query: &'a Query<'w, 's, &TransportBelt>,
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
        .map(|(_c, t)| {
            let dropoff_entities =
                find_entities_on_position(rapier_context, t.translation().xy(), None);
            dropoff_entities
        })
        .into_iter()
        .flatten()
        .flat_map(move |entity| {
            let inventory_dropoffs =
                find_inventory_dropoffs_for_entity(entity, inventories, &target_item);
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
    let pickups = find_pickups(
        inserter,
        inventories,
        belts_query,
        rapier_context,
        collider_query,
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
            pickups
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

    return pickup_valid;
}

fn inserter_planner(
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
                            &current_action,
                        )
                    });

            if new_action_needed {
                info!("Planning new action");
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
                inserter.current_action = new_action;
            }
        }
    }
}

pub fn inserter_tick(
    mut commands: Commands,
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter), With<Powered>>,
    time: Res<Time<Fixed>>,
    mut inventories: Query<&mut Inventory>,
    mut belts_query: Query<&mut TransportBelt>,
) {
    for (inserter_entity, _inserter_transform, mut inserter) in &mut inserter_query {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        if inserter.current_action.is_some() {
            commands.entity(inserter_entity).insert(Working);
        } else {
            commands.entity(inserter_entity).remove::<Working>();
        }

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

pub fn burner_inserter_tick(
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
