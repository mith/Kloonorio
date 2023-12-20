use bevy::{
    ecs::system::lifetimeless::{SQuery, SRes},
    math::Vec3Swizzles,
    prelude::*,
    reflect::Reflect,
};
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};

use crate::{
    inventory::{Fuel, Inventory, ItemFilter, Output, Source, Stack},
    types::{Powered, Product, Working},
    util::{
        drop_stack_at_point, get_inventory_child_mut, take_stack_from_entity_belt,
        try_get_inventory_child_mut, FuelInventoryQuery, Inventories, InventoryType,
    },
};

use super::{burner::Burner, transport_belt::TransportBelt};

pub struct InserterPlugin;

impl Plugin for InserterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, (inserter_tick, burner_inserter_tick))
            .register_type::<Inserter>();
    }
}

#[derive(Component, Debug, Reflect)]
pub struct Inserter {
    holding: Option<Stack>,
    capacity: u32,
    timer: Timer,
    dropoff_location_entity: Entity,
    pickup_location_entity: Entity,
    current_action: Option<InserterAction>,
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
            timer: Timer::from_seconds(speed, TimerMode::Repeating),
            dropoff_location_entity,
            pickup_location_entity,
            current_action: None,
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
    Filter(Vec<Name>),
}

impl InserterTargetItem {
    fn contains(&self, item: &Name) -> bool {
        match self {
            InserterTargetItem::Any => true,
            InserterTargetItem::Filter(filter) => filter.contains(item),
        }
    }

    fn to_filter(&self) -> ItemFilter {
        match self {
            InserterTargetItem::Any => ItemFilter::All,
            InserterTargetItem::Filter(filter) => {
                ItemFilter::Only(filter.iter().cloned().collect())
            }
        }
    }
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct DropoffRequest {
    target_type: InserterTargetType,
    target_item: InserterTargetItem,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct AvailablePickup {
    target_type: InserterTargetType,
    target_item: Name,
    count: u32,
}

#[derive(Hash, PartialEq, Eq, Clone, Debug, Reflect)]
struct InserterAction {
    pickup: Option<InserterTargetType>,
    dropoff: Option<InserterTargetType>,
    item: InserterTargetItem,
    max_stack_size: u32,
}

fn find_inventory_pickups_for_entity<'a>(
    entity: Entity,
    inventories: &'a Inventories,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 'a {
    [InventoryType::Output, InventoryType::Storage]
        .iter()
        .flat_map(move |&inventory_type| {
            inventories
                .get_inventory(entity, inventory_type)
                .into_iter()
                .flat_map(move |inventory| {
                    inventory.slots.iter().filter_map(move |slot| {
                        slot.as_ref().and_then(|stack| {
                            let stack_name = Name::new(stack.resource.to_string());
                            let is_target_item = match target_item {
                                InserterTargetItem::Any => true,
                                InserterTargetItem::Filter(ref filter) => {
                                    filter.contains(&stack_name)
                                }
                            };

                            if is_target_item {
                                Some(AvailablePickup {
                                    target_type: InserterTargetType::Inventory(entity),
                                    target_item: stack_name,
                                    count: stack.amount,
                                })
                            } else {
                                None
                            }
                        })
                    })
                })
        })
}

fn find_belt_pickups_for_entity<'w, 's, 'a>(
    entity: Entity,
    belts_query: &'a Query<'w, 's, &'a TransportBelt>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 's {
    belts_query
        .get(entity)
        .ok()
        .into_iter()
        .flat_map(move |belt| {
            belt.slots()[1].as_ref().and_then(|stack| {
                let stack_name = Name::new(stack.to_string());
                let is_target_item = match target_item {
                    InserterTargetItem::Any => true,
                    InserterTargetItem::Filter(ref filter) => filter.contains(&stack_name),
                };

                if is_target_item {
                    Some(AvailablePickup {
                        target_type: InserterTargetType::Belt(entity),
                        target_item: stack_name,
                        count: 1,
                    })
                } else {
                    None
                }
            })
        })
}

fn find_pickups<'w, 's, 'a, 'b>(
    inserter: &'a Inserter,
    inventories: &'a Inventories<'w, 's>,
    belts_query: &'a Query<'w, 's, &'a TransportBelt>,
    rapier_context: &'a Res<'w, RapierContext>,
    collider_query: &'a Query<'w, 's, (&'a Collider, &'a GlobalTransform)>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 's {
    collider_query
        .get(inserter.pickup_location_entity)
        .ok()
        .map(|(c, t)| {
            let mut pickup_entities = Vec::new();
            rapier_context.intersections_with_shape(
                t.translation().xy(),
                0.,
                c,
                QueryFilter::new().exclude_sensors(),
                |entity| {
                    pickup_entities.push(entity);
                    true
                },
            );
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
}

fn find_inventory_dropoffs_for_entity<'w, 's, 'a>(
    entity: Entity,
    inventories: &'a Inventories<'w, 's>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = DropoffRequest> + 'w + 's + 'a {
    [
        InventoryType::Fuel,
        InventoryType::Source,
        InventoryType::Storage,
    ]
    .iter()
    .filter_map(move |&inventory_type| {
        inventories
            .get_inventory(entity, inventory_type)
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
                        .map(|i| Name::new(i.to_string()))
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

fn find_dropoffs<'w, 's, 'a, 'b>(
    inserter: &'a Inserter,
    inventories: &'a Inventories<'w, 's>,
    belts_query: &'a Query<'w, 's, &'b TransportBelt>,
    collider_query: &'a Query<'w, 's, (&'b Collider, &'b GlobalTransform)>,
    rapier_context: &'a Res<'w, RapierContext>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = DropoffRequest> + 'w + 's + 'a + 'b {
    collider_query
        .get(inserter.dropoff_location_entity)
        .ok()
        .map(|(c, t)| {
            let mut dropoff_entities = Vec::new();
            rapier_context.intersections_with_shape(
                t.translation().xy(),
                0.,
                c,
                QueryFilter::new().exclude_sensors(),
                |entity| {
                    dropoff_entities.push(entity);
                    true
                },
            );
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
}

fn plan_inserter_action<'w, 's, 'a, 'b>(
    inserter: &'a Inserter,
    inventories: &'a Inventories<'w, 's>,
    collider_query: &'a Query<'w, 's, (&'b Collider, &'b GlobalTransform)>,
    belts_query: &'a Query<'w, 's, &'b TransportBelt>,
    rapier_context: &'a Res<'w, RapierContext>,
) -> Option<InserterAction> {
    let target_item = inserter
        .holding
        .as_ref()
        .map(|stack| InserterTargetItem::Filter(vec![Name::new(stack.resource.to_string())]))
        .unwrap_or(InserterTargetItem::Any);
    let mut dropoffs = find_dropoffs(
        inserter,
        inventories,
        belts_query,
        collider_query,
        rapier_context,
        &target_item,
    );
    if let Some(ref holding) = inserter.holding {
        dropoffs
            .find(|d| match d.target_item {
                InserterTargetItem::Any => true,
                InserterTargetItem::Filter(ref filter) => {
                    filter.contains(&Name::new(holding.resource.to_string()))
                }
            })
            .map(|dropoff| InserterAction {
                pickup: None,
                dropoff: Some(dropoff.target_type.clone()),
                item: dropoff.target_item.clone(),
                max_stack_size: inserter.capacity,
            })
    } else {
        // Iterate through each dropoff and try to find a matching pickup
        dropoffs.find_map(|dropoff| {
            find_pickups(
                inserter,
                inventories,
                belts_query,
                rapier_context,
                collider_query,
                &target_item,
            )
            .find(|pickup| match dropoff.target_item {
                InserterTargetItem::Any => true,
                InserterTargetItem::Filter(ref filter) => filter.contains(&pickup.target_item),
            })
            .map(|pickup| InserterAction {
                pickup: Some(pickup.target_type.clone()),
                dropoff: Some(dropoff.target_type.clone()),
                item: InserterTargetItem::Filter(vec![pickup.target_item.clone()]),
                max_stack_size: inserter.capacity,
            })
        })
    }
}

fn check_inserter_action_valid<'w, 's, 'a>(
    inserter: &'a Inserter,
    inventories: &'a Inventories<'w, 's>,
    belts_query: &'a Query<'w, 's, &TransportBelt>,
    rapier_context: &'a Res<'w, RapierContext>,
    collider_query: &'a Query<'w, 's, (&Collider, &GlobalTransform)>,
    action: &'a InserterAction,
) -> bool {
    let holding_valid = inserter
        .holding
        .as_ref()
        .map(|stack| action.item.contains(&Name::new(stack.resource.to_string())))
        .unwrap_or(true);

    if !holding_valid {
        return false;
    }

    let dropoff_valid = action.dropoff.as_ref().map_or(true, |dropoff| {
        match dropoff {
            InserterTargetType::Belt(entity) => belts_query
                .get(*entity)
                .ok()
                .map(|belt| belt.can_add(1))
                .unwrap_or(false),
            InserterTargetType::Inventory(entity) => inventories
                .get_inventory(*entity, InventoryType::Output)
                .map_or(false, |inventory| {
                    inventory.has_any_from_filter(&action.item.to_filter())
                }),
            InserterTargetType::ItemOnGround(entity) => {
                // TODO: Check if the ground is clear
                true
            }
        }
    });

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
                .map(|belt| belt.slots()[1].is_none())
                .unwrap_or(false),
            InserterTargetType::Inventory(entity) => inventories
                .get_inventory(*entity, InventoryType::Output)
                .map_or(false, |inventory| {
                    inventory.has_any_from_filter(&action.item.to_filter())
                }),
            InserterTargetType::ItemOnGround(entity) => {
                // Check if the item is still on the ground
                collider_query
                    .get(*entity)
                    .ok()
                    .and_then(|(c, t)| {
                        rapier_context.intersection_with_shape(
                            t.translation().xy(),
                            0.,
                            c,
                            QueryFilter::new()
                                .exclude_sensors()
                                .predicate(&|e| e != inserter.pickup_location_entity),
                        )
                    })
                    .is_some()
            }
        }
    });

    return pickup_valid;
}

fn inserter_planner(
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter), With<Powered>>,
    collider_query: Query<(&Collider, &GlobalTransform)>,
    rapier_context: Res<RapierContext>,
    inventories: Inventories,
    belts_query: Query<&TransportBelt>,
) {
    for (inserter_entity, _inserter_transform, mut inserter) in &mut inserter_query {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        {
            let new_action_needed =
                inserter
                    .current_action
                    .as_ref()
                    .map_or(true, |current_action| {
                        !check_inserter_action_valid(
                            &inserter,
                            &inventories,
                            &belts_query,
                            &rapier_context,
                            &collider_query,
                            &current_action,
                        )
                    });

            if new_action_needed {
                let new_action = plan_inserter_action(
                    &inserter,
                    &inventories,
                    &collider_query,
                    &belts_query,
                    &rapier_context,
                );
                inserter.current_action = new_action;
            }
        }
    }
}

pub fn inserter_tick(
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter), With<Powered>>,
    collider_query: Query<(&Collider, &GlobalTransform)>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    mut inventories_set: ParamSet<(
        Query<&mut Inventory, (Without<Fuel>, Without<Source>)>, // Pickup
        Query<&mut Inventory, Without<Output>>,                  // Dropoff
        Query<(Entity, &mut Inventory), Without<Output>>,        // Dropoff
        Inventories,
    )>,
    mut belts_set: ParamSet<(Query<&TransportBelt>, Query<&mut TransportBelt>)>,
) {
    for (inserter_entity, _inserter_transform, mut inserter) in &mut inserter_query {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        if let Some(ref action) = inserter.current_action {
            if inserter.timer.tick(time.delta()).just_finished() {
                if inserter.holding.is_some() {
                    // Dropoff
                    match action.dropoff.as_ref().unwrap() {
                        InserterTargetType::Belt(entity) => {
                            let stack = inserter.holding.take().unwrap();
                            let mut belt = belts_set.p1().get_mut(*entity).unwrap();
                            belt.add(1, stack.resource);
                        }
                        InserterTargetType::Inventory(entity) => {
                            let stack = inserter.holding.take().unwrap();
                            let mut inventory = inventories_set.p1().get_mut(*entity).unwrap();
                            inventory.add_stack(stack);
                        }
                        InserterTargetType::ItemOnGround(entity) => {
                            // TODO: Implement dropping items on the ground
                        }
                    }
                } else {
                    // Pickup
                    match action.pickup.as_ref().unwrap() {
                        InserterTargetType::Belt(entity) => {
                            let mut belt = belts_set.p1().get_mut(*entity).unwrap();
                            let stack = belt.slot_mut(1).unwrap().take().unwrap();
                            inserter.holding = Some(Stack {
                                resource: stack,
                                amount: 1,
                            });
                        }
                        InserterTargetType::Inventory(entity) => {
                            let mut inventory = inventories_set.p1().get_mut(*entity).unwrap();
                            let stack = inventory
                                .take_any_from_filter(&action.item.to_filter(), inserter.capacity);
                            inserter.holding = stack;
                        }
                        InserterTargetType::ItemOnGround(entity) => {
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
            if stack.resource == Product::Intermediate("Coal".into())
                && !fuel_inventory.has_items(&[(Product::Intermediate("Coal".into()), 2)])
            {
                info!("Taking fuel from hand to refuel");
                fuel_inventory.add_stack(stack.to_owned());
                inserter.holding = None;
            }
        }
    }
}
