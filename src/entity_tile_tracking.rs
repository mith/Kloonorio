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
        app.register_type::<EntityOnTile>()
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
struct EntityOnTile {
    tile_entity: Entity,
}

impl EntityOnTile {
    pub fn tile_entity(&self) -> Entity {
        self.tile_entity
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
        (Option<&mut EntityOnTile>, &GlobalTransform, Entity),
        Or<(
            (With<TileTracked>, Changed<GlobalTransform>),
            Added<TileTracked>,
        )>,
    >,
    tile_occupants_query: Query<(Entity, &TileOccupants)>,
    terrain_params: TerrainParams,
    tile_pos_query: Query<&TilePos>,
) {
    let mut tile_occupants: HashMap<Entity, HashSet<Entity>> = tile_occupants_query
        .iter()
        .map(|(e, o)| (e, o.occupants.iter().cloned().collect()))
        .collect();
    for (mut entity_on_tile, global_transform, entity) in query.iter_mut() {
        let entity_position = global_transform.translation().xy();
        let Some(new_tile_entity) = terrain_params.tile_entity_at_global_pos(entity_position)
        else {
            if let Some(entity_on_tile) = entity_on_tile {
                remove_from_tile_occupants(&mut tile_occupants, entity_on_tile.tile_entity, entity);
                commands.entity(entity).remove::<EntityOnTile>();
            }
            continue;
        };

        let new_tile_pos = tile_pos_query.get(new_tile_entity).unwrap();

        match entity_on_tile.as_mut() {
            Some(entity_on_tile) => {
                let old_tile_entity = entity_on_tile.tile_entity;
                remove_from_tile_occupants(&mut tile_occupants, old_tile_entity, entity);
                add_to_tile_occupants(&mut tile_occupants, new_tile_entity, entity);
                entity_on_tile.tile_entity = new_tile_entity;
            }
            None => {
                commands.entity(entity).insert(EntityOnTile {
                    tile_entity: new_tile_entity,
                });
                add_to_tile_occupants(&mut tile_occupants, new_tile_entity, entity);
            }
            _ => {}
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
        .or_insert_with(HashSet::new)
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
