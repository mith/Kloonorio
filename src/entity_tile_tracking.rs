use bevy::{
    app::{App, Plugin},
    ecs::{
        component::Component,
        entity::Entity,
        query::{Changed, With},
        removal_detection::RemovedComponents,
        system::{Commands, Query},
    },
    math::Vec3Swizzles,
    transform::components::GlobalTransform,
    utils::HashSet,
};
use bevy_rapier2d::geometry::Collider;

use crate::terrain::TerrainParams;

pub struct EntityTileTrackingPlugin;

impl Plugin for EntityTileTrackingPlugin {
    fn build(&self, app: &mut App) {}
}

#[derive(Component)]
struct TileTracked;

#[derive(Component)]
struct EntityOnTile {
    tile_entity: Entity,
}

#[derive(Component, Default)]
struct TileOccupants {
    occupants: HashSet<Entity>,
}

fn update_entity_on_tile_system(
    mut commands: Commands,
    mut query: Query<
        (Option<&mut EntityOnTile>, &GlobalTransform, Entity),
        (With<TileTracked>, Changed<GlobalTransform>),
    >,
    mut tile_query: Query<&mut TileOccupants>,
    terrain_params: TerrainParams,
) {
    for (mut entity_on_tile, global_transform, entity) in query.iter_mut() {
        let new_tile_entity = match terrain_params
            .tile_entity_at_global_pos(global_transform.translation().xy())
        {
            Some(tile_entity) => tile_entity,
            None => {
                if let Some(entity_on_tile) = entity_on_tile {
                    remove_from_tile_occupants(&mut tile_query, entity_on_tile.tile_entity, entity);
                    commands.entity(entity).remove::<EntityOnTile>();
                }
                continue;
            }
        };

        match entity_on_tile.as_mut() {
            Some(entity_on_tile) if new_tile_entity != entity_on_tile.tile_entity => {
                remove_from_tile_occupants(&mut tile_query, entity_on_tile.tile_entity, entity);
                add_to_tile_occupants(&mut tile_query, new_tile_entity, entity);
                entity_on_tile.tile_entity = new_tile_entity;
            }
            None => {
                commands.entity(entity).insert(EntityOnTile {
                    tile_entity: new_tile_entity,
                });
                add_to_tile_occupants(&mut tile_query, new_tile_entity, entity);
            }
            _ => {}
        }
    }
}

fn add_to_tile_occupants(
    tile_query: &mut Query<&mut TileOccupants>,
    tile_entity: Entity,
    entity: Entity,
) {
    if let Ok(mut tile_occupants) = tile_query.get_mut(tile_entity) {
        tile_occupants.occupants.insert(entity);
    }
}

fn remove_from_tile_occupants(
    tile_query: &mut Query<&mut TileOccupants>,
    tile_entity: Entity,
    entity: Entity,
) {
    if let Ok(mut tile_occupants) = tile_query.get_mut(tile_entity) {
        tile_occupants.occupants.remove(&entity);
    }
}

fn cleanup_on_removal_system(
    mut commands: Commands,
    removed_entities: RemovedComponents<EntityOnTile>,
    mut tile_query: Query<&mut TileOccupants>,
) {
    // Implementation to remove entities from TileOccupants if they are removed from the game
}
