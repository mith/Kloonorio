use bevy::{
    app::{App, Plugin},
    ecs::component::Component,
    prelude::Entity,
    reflect::Reflect,
    utils::HashSet,
};

pub struct TileOccupantsPlugin;

impl Plugin for TileOccupantsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TileOccupants>()
            .register_type::<EntityOnTiles>();
    }
}

#[derive(Component, Debug, Reflect)]
pub struct EntityOnTiles(Vec<Entity>);

impl EntityOnTiles {
    pub fn new(tile_entities: Vec<Entity>) -> Self {
        EntityOnTiles(tile_entities)
    }
    pub fn tile_entities(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }
}

#[derive(Component, Default, Debug, Reflect)]
pub struct TileOccupants(HashSet<Entity>);

impl TileOccupants {
    pub fn new(occupants: HashSet<Entity>) -> Self {
        Self(occupants)
    }

    pub fn add(&mut self, entity: Entity) {
        self.0.insert(entity);
    }

    pub fn remove(&mut self, entity: &Entity) {
        self.0.remove(entity);
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.0.contains(entity)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.0.iter()
    }
}
