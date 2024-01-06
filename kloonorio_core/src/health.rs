use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query},
    },
    hierarchy::DespawnRecursiveExt,
    reflect::Reflect,
};

use crate::types::AppState;

pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Health>()
            .add_systems(Update, health.run_if(in_state(AppState::Running)));
    }
}

#[derive(Component, Reflect)]
pub struct Health {
    current: u32,
    max: u32,
}

impl Health {
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }

    pub fn current(&self) -> u32 {
        self.current
    }

    pub fn max(&self) -> u32 {
        self.max
    }

    pub fn damage(&mut self, damage: u32) {
        self.current = self.current.saturating_sub(damage);
    }
}

fn health(mut commands: Commands, health_query: Query<(Entity, &Health)>) {
    for (entity, health) in &health_query {
        if health.current == 0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}
