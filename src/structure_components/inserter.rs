use bevy::{math::Vec3Swizzles, prelude::*, reflect::Reflect};
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

fn find_belt_pickups_for_entity<'a>(
    entity: Entity,
    belts_query: &'a Query<&'static TransportBelt>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 'a {
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

fn find_pickups<'a>(
    inserter: &'a Inserter,
    inventories: &'a Inventories,
    belts_query: &'a Query<&'static TransportBelt>,
    rapier_context: &'a Res<RapierContext>,
    collider_query: &'a Query<(&'static Collider, &'static GlobalTransform)>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = AvailablePickup> + 'a {
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

fn find_inventory_dropoffs_for_entity<'a>(
    entity: Entity,
    inventories: &'a Inventories,
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

fn find_belt_dropoffs_for_entity<'a>(
    entity: Entity,
    belts_query: &'a Query<&'static TransportBelt>,
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

fn find_dropoffs<'a>(
    inserter: &'a Inserter,
    inventories: &'a Inventories,
    belts_query: &'a Query<&'static TransportBelt>,
    collider_query: &'a Query<(&'static Collider, &'static GlobalTransform)>,
    rapier_context: &'a Res<RapierContext>,
    target_item: &'a InserterTargetItem,
) -> impl Iterator<Item = DropoffRequest> + 'a {
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

fn plan_inserter_action<'w, 's, 'a>(
    inserter: &'a Inserter,
    inventories: &'a Inventories,
    collider_query: &'a Query<'w, 's, (&'static Collider, &'static GlobalTransform)>,
    belts_query: &'a Query<'w, 's, &'static TransportBelt>,
    rapier_context: &'a Res<RapierContext>,
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

pub fn inserter_tick(
    mut commands: Commands,
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter, &Children), With<Powered>>,
    pickup_query: Query<(Entity, &GlobalTransform, &Collider), With<Pickup>>,
    dropoff_query: Query<(Entity, &GlobalTransform, &Collider), With<Dropoff>>,
    children: Query<&Children>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    mut inventories_set: ParamSet<(
        Query<&mut Inventory, (Without<Fuel>, Without<Source>)>, // Pickup
        Query<&mut Inventory, Without<Output>>,                  // Dropoff
        Query<(Entity, &mut Inventory), Without<Output>>,        // Dropoff
        Inventories,
    )>,
    mut belts_query: Query<&mut TransportBelt>,
    asset_server: Res<AssetServer>,
    name_query: Query<&Name>,
) {
    for (inserter_entity, _inserter_transform, mut inserter, inserter_children) in
        &mut inserter_query
    {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        let Some(drop_point) = inserter_children
            .iter()
            .find(|c| dropoff_query.get(**c).is_ok())
            .and_then(|e| dropoff_query.get(*e).ok())
        else {
            commands.entity(inserter_entity).remove::<Working>();
            continue;
        };

        if let Some(holding) = inserter.holding.clone() {
            if inserter.timer.tick(time.delta()).just_finished() {
                let drop_point_transform = drop_point.1;
                if drop_stack_at_point(
                    &mut commands,
                    &rapier_context,
                    &asset_server,
                    &mut inventories_set.p1(),
                    &mut belts_query,
                    &children,
                    holding.clone(),
                    drop_point_transform.translation(),
                ) {
                    info!(dropped = ?holding, "Dropped stack");
                    inserter.holding = None;
                }
            }
        } else {
            let allowed_products: Vec<(Entity, ItemFilter)> = inventories_set
                .p2()
                .iter()
                .map(|(e, inventory)| (e, inventory.allowed_items.clone()))
                .collect();

            let Some((_0, pickup_point_transform, pickup_collider)) = inserter_children
                .iter()
                .find(|c| pickup_query.get(**c).is_ok())
                .and_then(|e| pickup_query.get(*e).ok())
            else {
                commands.entity(inserter_entity).remove::<Working>();
                continue;
            };

            if let Some(collider_entity) = rapier_context.intersection_with_shape(
                pickup_point_transform.translation().xy(),
                0.,
                pickup_collider,
                QueryFilter::new().exclude_sensors(),
            ) {
                let collider_name = name_query.get(collider_entity).unwrap();
                debug!(collider = ?collider_name, "Found collider");

                // Check if the collider has an inventory that contains a product that is allowed
                // by the dropoff inventory
                let Some(collider_children) = children.get(collider_entity).ok() else {
                    commands.entity(inserter_entity).remove::<Working>();
                    continue;
                };
                let mut p3 = inventories_set.p3();

                let Some((source_inventory_entity, dropoff_entity, filter)) = [
                    try_get_inventory_child_mut(&collider_children, &mut p3.output_inventories),
                    try_get_inventory_child_mut(&collider_children, &mut p3.storage_inventories),
                ]
                .iter_mut()
                .filter_map(|x| x.as_ref())
                .find_map(|(source_inventory_entity, inventory)| {
                    allowed_products
                        .iter()
                        .find_map(|(dropoff_entity, filter)| {
                            if inventory.has_any_from_filter(filter) {
                                Some((*source_inventory_entity, *dropoff_entity, filter.clone()))
                            } else {
                                None
                            }
                        })
                }) else {
                    commands.entity(inserter_entity).remove::<Working>();
                    continue;
                };

                inserter.holding = ({
                    let p0 = &mut inventories_set.p0();
                    let mut source_inventory = p0.get_mut(source_inventory_entity).unwrap();
                    source_inventory.take_any_from_filter(&filter, inserter.capacity)
                })
                .or_else(|| {
                    take_stack_from_entity_belt(
                        &mut belts_query,
                        collider_entity,
                        inserter.capacity,
                    )
                });
                if inserter.holding.is_some() {
                    info!(stack = ?inserter.holding, "Picked up stack");
                    commands.entity(inserter_entity).insert(Working);
                } else {
                    commands.entity(inserter_entity).remove::<Working>();
                }
            } else {
                commands.entity(inserter_entity).remove::<Working>();
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
