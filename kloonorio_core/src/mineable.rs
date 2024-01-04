use bevy::ecs::component::Component;

use crate::item::Item;

#[derive(Component)]
pub struct Mineable(pub Item);
