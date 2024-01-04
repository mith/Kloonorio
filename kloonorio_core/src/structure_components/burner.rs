use bevy::prelude::*;

use crate::{
    inventory::{Fuel, Inventory},
    item::Item,
    types::{Powered, Working},
};

#[derive(Component, Default)]
pub struct Burner {
    pub fuel_timer: Option<Timer>,
}

pub fn burner_tick(
    mut commands: Commands,
    mut burner_query: Query<(Entity, &mut Burner), With<Working>>,
    time: Res<Time>,
) {
    for (entity, mut burner) in &mut burner_query {
        if let Some(timer) = &mut burner.fuel_timer {
            if timer.tick(time.delta()).just_finished() {
                commands.entity(entity).remove::<Powered>();
                burner.fuel_timer = None;
            }
        }
    }
}

pub fn burner_load(
    mut commands: Commands,
    mut fueled_query: Query<(Entity, &mut Burner, &Children), Without<Powered>>,
    mut fuel_inventory_query: Query<&mut Inventory, With<Fuel>>,
) {
    for (entity, mut fueled, children) in &mut fueled_query {
        for child in children {
            if let Ok(mut fuel_inventory) = fuel_inventory_query.get_mut(*child) {
                if fuel_inventory.remove_items(&[(Item::new("Coal"), 1)]) {
                    fueled.fuel_timer = Some(Timer::from_seconds(10., TimerMode::Once));
                    commands.entity(entity).insert(Powered);
                    break;
                }
            }
        }
    }
}
