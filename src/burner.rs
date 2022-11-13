use bevy::prelude::*;

use crate::{
    inventory::Inventory,
    types::{Powered, Resource, Working},
};

#[derive(Component)]
pub struct Burner {
    pub fuel_inventory: Inventory,
    pub fuel_timer: Option<Timer>,
}

impl Burner {
    pub fn new() -> Self {
        Self {
            fuel_inventory: Inventory::new(1),
            fuel_timer: None,
        }
    }
}

pub fn burner_tick(
    mut commands: Commands,
    mut fueled_query: Query<(Entity, &mut Burner), With<Working>>,
    time: Res<Time>,
) {
    for (entity, mut fueled) in fueled_query.iter_mut() {
        if let Some(timer) = &mut fueled.fuel_timer {
            if timer.tick(time.delta()).just_finished() {
                commands.entity(entity).remove::<Powered>();
                fueled.fuel_timer = None;
            }
        }
    }
}

pub fn burner_load(
    mut commands: Commands,
    mut fueled_query: Query<(Entity, &mut Burner), Without<Powered>>,
) {
    for (entity, mut fueled) in fueled_query.iter_mut() {
        if fueled.fuel_inventory.remove_items(&[(Resource::Coal, 1)]) {
            fueled.fuel_timer = Some(Timer::from_seconds(10., false));
            commands.entity(entity).insert(Powered);
        }
    }
}
