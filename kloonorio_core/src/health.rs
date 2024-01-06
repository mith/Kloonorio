use crate::types::AppState;
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

pub struct HealthPlugin;

impl Plugin for HealthPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Health>()
            .add_systems(Update, health.run_if(in_state(AppState::Running)));
    }
}

#[derive(Component, Reflect)]
pub struct Health {
    pub current: u32,
    pub max: u32,
}

impl Health {
    pub fn new(max: u32) -> Self {
        Self { current: max, max }
    }
}

fn health(mut commands: Commands, health_query: Query<(Entity, &Health)>) {
    for (entity, health) in &health_query {
        if health.current == 0 {
            commands.entity(entity).despawn_recursive();
        }
    }
}
