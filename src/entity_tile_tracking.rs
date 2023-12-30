use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        query::{Added, Changed, Or, With},
        removal_detection::RemovedComponents,
        schedule::{IntoSystemConfigs, SystemSet},
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
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (update_entity_on_tile_system, tile_tracker_removed).in_set(EntityTileTrackingSet),
        );
    }
}

#[derive(SystemSet, Default, Hash, PartialEq, Eq, Debug, Clone)]
pub struct EntityTileTrackingSet;

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
        Or<(
            (With<TileTracked>, Changed<GlobalTransform>),
            Added<TileTracked>,
        )>,
    >,
    mut tile_query: Query<&mut TileOccupants>,
    terrain_params: TerrainParams,
) {
    for (mut entity_on_tile, global_transform, entity) in query.iter_mut() {
        let Some(new_tile_entity) =
            terrain_params.tile_entity_at_global_pos(global_transform.translation().xy())
        else {
            if let Some(entity_on_tile) = entity_on_tile {
                remove_from_tile_occupants(&mut tile_query, entity_on_tile.tile_entity, entity);
                commands.entity(entity).remove::<EntityOnTile>();
            }
            continue;
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

fn tile_tracker_removed(
    mut removed_tracker: RemovedComponents<TileTracked>,
    mut tile_query: Query<&mut TileOccupants>,
) {
    for entity in removed_tracker.read() {
        if let Ok(mut entity_on_tile) = tile_query.get_mut(entity) {
            entity_on_tile.occupants.remove(&entity);
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{
        terrain::{
            terrain_generator::{FlatChunkGenerator, TerrainGenerator},
            TerrainBundle, TerrainPlugin, GROUND,
        },
        types::AppState,
    };

    use super::*;
    use bevy::{app::Update, ecs::schedule::State, ecs::system::SystemState, math::IVec2};
    use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};

    prop_compose! {
        fn any_ivec2(min: i32, max: i32)
                    (x in min..=max, y in min..=max)
                    -> IVec2 {
            IVec2::new(x, y)

        }
    }

    #[derive(Component)]
    struct InitialTilePosition {
        tile_position: IVec2,
    }

    proptest! {
        #[test]
        fn test_update_entity_on_tile_system(
            entities_positions in prop::collection::vec(
                any_ivec2(-4, 4),
                5
            ),
        ) {
            let mut app = App::new();

            // Setup terrain
            let terrain_entity = app.world.spawn(TerrainBundle::default()).id();

            {
                // Get system state to trigger spawning chunk (0, 0)
                let mut system_state: SystemState<TerrainParams> = SystemState::new(&mut app.world);
                let mut terrain_params = system_state.get_mut(&mut app.world);
                terrain_params.queue_spawn_chunk(terrain_entity, IVec2::new(0, 0));
            }

            app.add_systems(Update, update_entity_on_tile_system);

            // Setup entities with initial positions
            for pos in entities_positions {
                let entity_transform = GlobalTransform::from_translation(pos.as_vec2().extend(0.0));
                let initial_position = InitialTilePosition {
                    tile_position: pos,
                };
                let entity = app.world.spawn((entity_transform, initial_position)).id();
            }

            app.update();

            // Check that entities are correctly assigned to tiles
            {
                let mut system_state: SystemState<(
                    Query<(Entity, &InitialTilePosition)>,
                    Query<&TileOccupants>,
                    TerrainParams
                )> = SystemState::new(&mut app.world);
                let (entity_query, tile_query, terrain_params) = system_state.get_mut(&mut app.world);

                for (entity, initial_position) in entity_query.iter() {
                    let tile_entity = terrain_params.tile_entity_at_global_pos(initial_position.tile_position.as_vec2()).unwrap();
                    let tile_occupants = tile_query.get(tile_entity).unwrap();
                    assert!(tile_occupants.occupants.contains(&entity), "Entity should be in the tile occupants");
                }

            }


            // // Simulate entity movement
            // for (entity, _) in entity_transforms.iter_mut() {
            //     // Randomly assign a new position to simulate movement
            //     let new_pos = any_ivec2(-4, 4).new_tree(&mut TestRunner::default()).unwrap().current();
            //     app.world.entity_mut(*entity).insert(GlobalTransform::from_translation(new_pos.as_vec2().extend(0.0)));
            // }

            // // Verify the correctness
            // for (entity, transform) in entity_transforms {
            //     let tile_occupants = app.world.get::<TileOccupants>(tile_entity).unwrap();
            //     assert!(tile_occupants.occupants.contains(&entity), "Entity should be in the tile occupants");
            // }
        }
    }
}
