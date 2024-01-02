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
    math::{Vec2, Vec3Swizzles},
    reflect::Reflect,
    transform::{commands, components::GlobalTransform},
    utils::{HashMap, HashSet},
};
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::geometry::Collider;
use tracing::info;

use crate::terrain::TerrainParams;

pub struct EntityTileTrackingPlugin;

impl Plugin for EntityTileTrackingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<EntityOnTiles>()
            .register_type::<TileOccupants>()
            .add_systems(
                Update,
                (update_entity_on_tile_system, tile_tracker_removed).in_set(EntityTileTrackingSet),
            );
    }
}

#[derive(SystemSet, Default, Hash, PartialEq, Eq, Debug, Clone)]
pub struct EntityTileTrackingSet;

#[derive(Component)]
pub struct TileTracked;

#[derive(Component, Debug, Reflect)]
struct EntityOnTiles {
    tile_entities: Vec<Entity>,
}

impl EntityOnTiles {
    pub fn tile_entities(&self) -> impl Iterator<Item = &Entity> {
        self.tile_entities.iter()
    }
}

#[derive(Component, Default, Debug, Reflect)]
pub struct TileOccupants {
    occupants: HashSet<Entity>,
}

impl TileOccupants {
    #[cfg(test)]
    pub fn new(occupants: &[Entity]) -> Self {
        TileOccupants {
            occupants: occupants.iter().cloned().collect(),
        }
    }

    #[cfg(test)]
    pub fn add(&mut self, entity: Entity) {
        self.occupants.insert(entity);
    }

    pub fn contains(&self, entity: &Entity) -> bool {
        self.occupants.contains(entity)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Entity> {
        self.occupants.iter()
    }
}

fn update_entity_on_tile_system(
    mut commands: Commands,
    mut query: Query<
        (
            Option<&mut EntityOnTiles>,
            &GlobalTransform,
            Entity,
            Option<&Collider>,
        ),
        Or<(
            (With<TileTracked>, Changed<GlobalTransform>),
            Added<TileTracked>,
        )>,
    >,
    tile_occupants_query: Query<(Entity, &TileOccupants)>,
    terrain_params: TerrainParams,
) {
    let mut tile_occupants: HashMap<Entity, HashSet<Entity>> = tile_occupants_query
        .iter()
        .map(|(e, o)| (e, o.occupants.iter().cloned().collect()))
        .collect();
    for (mut entity_on_tile, global_transform, entity, opt_collider) in query.iter_mut() {
        // Find the new tiles the entity is on
        let entity_position = global_transform.translation().xy();
        let entity_tile_positions = {
            if let Some(collider) = opt_collider {
                let aabb = collider.raw.compute_aabb(&entity_position.into());
                let min_x = aabb.mins.x.floor() as i32;
                let min_y = aabb.mins.y.floor() as i32;
                let max_x = aabb.maxs.x.ceil() as i32;
                let max_y = aabb.maxs.y.ceil() as i32;
                let mut tiles = Vec::new();
                for x in min_x..=max_x {
                    for y in min_y..=max_y {
                        let tile_global_pos = Vec2::new(x as f32, y as f32);
                        tiles.push(tile_global_pos);
                    }
                }
                tiles
            } else {
                vec![entity_position]
            }
        };

        if let Some(entity_on_tile) = entity_on_tile.as_mut() {
            // Remove the entity from the occupant list of the old tiles
            for tile_entity in entity_on_tile.tile_entities() {
                remove_from_tile_occupants(&mut tile_occupants, *tile_entity, entity);
            }
            // Update the entity's tile list
            entity_on_tile.tile_entities = entity_tile_positions
                .iter()
                .filter_map(|tile_global_pos| {
                    terrain_params.tile_entity_at_global_pos(*tile_global_pos)
                })
                .collect();
            for tile_entity in entity_on_tile.tile_entities() {
                add_to_tile_occupants(&mut tile_occupants, *tile_entity, entity);
            }
        } else {
            let tile_entities = entity_tile_positions
                .iter()
                .filter_map(|tile_global_pos| {
                    terrain_params.tile_entity_at_global_pos(*tile_global_pos)
                })
                .collect::<Vec<_>>();
            for tile_entity in &tile_entities {
                add_to_tile_occupants(&mut tile_occupants, *tile_entity, entity);
            }
            commands
                .entity(entity)
                .insert(EntityOnTiles { tile_entities });
        }
    }

    for (tile_entity, occupants) in tile_occupants {
        if occupants.is_empty() {
            commands.entity(tile_entity).remove::<TileOccupants>();
        } else {
            commands
                .entity(tile_entity)
                .insert(TileOccupants { occupants });
        }
    }
}

fn add_to_tile_occupants(
    tile_occupants: &mut HashMap<Entity, HashSet<Entity>>,
    tile_entity: Entity,
    entity: Entity,
) {
    tile_occupants
        .entry(tile_entity)
        .or_default()
        .insert(entity);
}

fn remove_from_tile_occupants(
    tile_occupants: &mut HashMap<Entity, HashSet<Entity>>,
    tile_entity: Entity,
    entity: Entity,
) {
    if let Some(tile_occupants) = tile_occupants.get_mut(&tile_entity) {
        tile_occupants.remove(&entity);
    }
}

fn tile_tracker_removed(
    mut commands: Commands,
    mut removed_tracker: RemovedComponents<TileTracked>,
    entity_on_tiles_query: Query<&EntityOnTiles>,
    mut tile_occupants_query: Query<&mut TileOccupants>,
) {
    for entity in removed_tracker.read() {
        if let Ok(entity_on_tiles) = entity_on_tiles_query.get(entity) {
            for tile_entity in entity_on_tiles.tile_entities() {
                if let Ok(mut tile_occupants) = tile_occupants_query.get_mut(*tile_entity) {
                    tile_occupants.occupants.remove(&entity);
                }
            }
        }
        commands.entity(entity).remove::<EntityOnTiles>();
    }
}

#[cfg(test)]
mod test {
    use crate::{
        terrain::{
            terrain_generator::{FlatChunkGenerator, TerrainGenerator},
            test::spawn_test_terrain,
            ChunkBundle, TerrainBundle, TerrainPlugin, GROUND,
        },
        types::AppState,
    };

    use super::*;
    use bevy::{
        app::Update,
        ecs::schedule::State,
        ecs::system::SystemState,
        math::{IVec2, Vec2},
        transform::TransformPlugin,
    };
    use bevy_ecs_tilemap::tiles::TilePos;
    use proptest::{prelude::*, strategy::ValueTree, test_runner::TestRunner};

    prop_compose! {
        fn any_ivec2(min: i32, max: i32)
                    (x in min..=max, y in min..=max)
                    -> IVec2 {
            IVec2::new(x, y)

        }
    }

    #[derive(Component)]
    struct InitialTilePosition(IVec2);

    proptest! {
        #[test]
        fn test_update_entity_on_tile_system(
            entities_positions in prop::collection::vec(
                any_ivec2(-4, 4),
                5
            ),
        ) {
            let mut app = App::new();
            app.add_plugins(TransformPlugin);

             // Setup terrain
            let _terrain_entity = spawn_test_terrain(&mut app);

            // Setup entities with initial positions
            for pos in entities_positions {
                let entity_transform = GlobalTransform::from_translation(pos.as_vec2().extend(0.0));
                let initial_position = InitialTilePosition(pos);
                app.world.spawn((TileTracked, entity_transform, initial_position));
            }
            app.update();

            app.add_systems(Update, update_entity_on_tile_system);
            app.update();

            // Check that entities are correctly assigned to tiles
            {
                let mut system_state: SystemState<(
                    Query<(Entity, &InitialTilePosition, &GlobalTransform)>,
                    Query<(&TileOccupants, &TilePos)>,
                    TerrainParams
                )> = SystemState::new(&mut app.world);
                let (entity_query, tile_query, terrain_params) = system_state.get_mut(&mut app.world);

                for (entity, initial_position, _entity_transform) in entity_query.iter() {
                    let tile_entity = terrain_params.tile_entity_at_global_pos(initial_position.0.as_vec2()).unwrap();
                    let (tile_occupants, _tile_pos) = tile_query.get(tile_entity).unwrap();
                    let tile_occupants_vec = tile_occupants.occupants.iter().cloned().collect::<Vec<_>>();
                    let _occupants_initial_pos = tile_occupants_vec.iter().map(|e| {
                        let (_, initial_position, entity_transform) = entity_query.get(*e).unwrap();
                        (initial_position.0, entity_transform.translation().xy())
                    }).collect::<Vec<_>>();
                    assert!(tile_occupants.occupants.contains(&entity), "Entity should be in the tile occupants");
                }
            }
        }
    }
}
