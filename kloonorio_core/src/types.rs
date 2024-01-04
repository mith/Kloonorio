use std::collections::VecDeque;

use bevy::prelude::*;

use crate::recipe::Recipe;

#[derive(Clone, PartialEq, Eq, Component, Debug, Hash, States, Default, Reflect)]
pub enum AppState {
    #[default]
    Loading,
    Running,
}

#[derive(Component)]
pub struct StaticDimensions(pub IVec2);

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Powered;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Working;

#[derive(Component, Default, Reflect)]
pub struct CraftingQueue(pub VecDeque<ActiveCraft>);

#[derive(Reflect)]
pub struct ActiveCraft {
    pub recipe: Recipe,
    pub timer: Timer,
}

#[derive(Component)]
pub struct Ghost;

#[derive(Component)]
pub struct Building;

#[derive(Component)]
pub struct Pickup;

#[derive(Component)]
pub struct Dropoff;

#[derive(Component)]
pub struct MineCountdown {
    pub timer: Timer,
    pub target: Entity,
}
