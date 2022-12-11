use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};

use crate::{
    burner::Burner,
    inventory::{Fuel, Inventory, Output, Source, Stack},
    types::{Powered, Product, Working},
    util::{
        drop_stack_at_point, get_inventory_child_mut, take_stack_from_entity_inventory,
        FuelInventoryQuery,
    },
};

#[derive(Component)]
pub struct Inserter {
    holding: Option<Stack>,
    speed: f32,
    capacity: u32,
    timer: Timer,
}

impl Inserter {
    pub fn new(speed: f32, capacity: u32) -> Self {
        Inserter {
            holding: None,
            speed,
            capacity,
            timer: Timer::from_seconds(speed, TimerMode::Repeating),
        }
    }
}

#[derive(Component)]
pub struct Pickup;

#[derive(Component)]
pub struct Dropoff;

pub fn inserter_tick(
    mut commands: Commands,
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter, &Children), With<Powered>>,
    pickup_query: Query<(Entity, &GlobalTransform, &Collider), With<Pickup>>,
    dropoff_query: Query<(Entity, &GlobalTransform, &Collider), With<Dropoff>>,
    children: Query<&Children>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    mut inventories_set: ParamSet<(
        Query<&mut Inventory, (Without<Fuel>, Without<Source>)>,
        Query<&mut Inventory, Without<Output>>,
    )>,
    asset_server: Res<AssetServer>,
    name_query: Query<&Name>,
) {
    for (inserter_entity, _inserter_transform, mut inserter, inserter_children) in
        &mut inserter_query
    {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        if let Some(holding) = inserter.holding.clone() {
            if inserter.timer.tick(time.delta()).just_finished() {
                if let Some(drop_point_entity) = inserter_children
                    .iter()
                    .find(|c| dropoff_query.get(**c).is_ok())
                {
                    let drop_point_transform = dropoff_query.get(*drop_point_entity).unwrap().1;
                    if drop_stack_at_point(
                        &mut commands,
                        &rapier_context,
                        &asset_server,
                        &mut inventories_set.p1(),
                        &children,
                        holding.clone(),
                        drop_point_transform.translation(),
                    ) {
                        info!(dropped = ?holding, "Dropped stack");
                        inserter.holding = None;
                    }
                }
            }
        } else {
            if let Some(pickup_point_entity) = inserter_children
                .iter()
                .find(|c| pickup_query.get(**c).is_ok())
            {
                let (_, pickup_point_transform, pickup_collider) =
                    pickup_query.get(*pickup_point_entity).unwrap();
                if let Some(collider_entity) = rapier_context.intersection_with_shape(
                    pickup_point_transform.translation().xy(),
                    0.,
                    pickup_collider,
                    QueryFilter::new().exclude_sensors(),
                ) {
                    let collider_name = name_query.get(collider_entity).unwrap();
                    info!(collider = ?collider_name, "Found collider");
                    inserter.holding = take_stack_from_entity_inventory(
                        &mut inventories_set.p0(),
                        collider_entity,
                        &children,
                        inserter.capacity,
                    );
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
