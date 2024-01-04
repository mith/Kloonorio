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
    transform::components::GlobalTransform,
    utils::{HashMap, HashSet},
};

use bevy_rapier2d::geometry::Collider;
use kloonorio_core::tile_occupants::{EntityOnTiles, TileOccupants};
use kloonorio_terrain::TerrainParams;

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
pub struct TileTracked;

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
        .map(|(tile_entity, occupants)| (tile_entity, occupants.iter().cloned().collect()))
        .collect();
    for (mut entity_on_tile, global_transform, entity, opt_collider) in query.iter_mut() {
        // Find the new tiles the entity is on
        let entity_position = global_transform.translation().xy();
        let entity_tile_positions = {
            if let Some(collider) = opt_collider {
                get_covered_tiles_for_collider(collider, entity_position)
            } else {
                vec![entity_position]
            }
        };

        if let Some(entity_on_tile) = entity_on_tile.as_mut() {
            // Remove the entity from the occupant list of the old tiles
            for tile_entity in entity_on_tile.tile_entities() {
                remove_from_tile_occupants(&mut tile_occupants, *tile_entity, entity);
            }
        }

        let tile_entities = entity_tile_positions
            .iter()
            .filter_map(|tile_global_pos| {
                terrain_params.tile_entity_at_global_pos(*tile_global_pos)
            })
            .collect::<Vec<_>>();
        for tile_entity in &tile_entities {
            add_to_tile_occupants(&mut tile_occupants, *tile_entity, entity);
        }
        if entity_tile_positions.is_empty() {
            commands.entity(entity).remove::<EntityOnTiles>();
        } else {
            commands
                .entity(entity)
                .insert(EntityOnTiles::new(tile_entities));
        }
    }

    for (tile_entity, occupants) in tile_occupants {
        if occupants.is_empty() {
            commands.entity(tile_entity).remove::<TileOccupants>();
        } else {
            commands
                .entity(tile_entity)
                .insert(TileOccupants::new(occupants));
        }
    }
}

fn get_covered_tiles_for_collider(collider: &Collider, entity_position: Vec2) -> Vec<Vec2> {
    let aabb = collider.raw.compute_aabb(&entity_position.into());
    let min_x = aabb.mins.x.round() as i32;
    let min_y = aabb.mins.y.round() as i32;
    let max_x = aabb.maxs.x.round() as i32;
    let max_y = aabb.maxs.y.round() as i32;
    let mut tiles = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let tile_global_pos = Vec2::new(x as f32, y as f32);
            tiles.push(tile_global_pos);
        }
    }
    tiles
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
                    tile_occupants.remove(&entity);
                }
            }
        }
        commands.entity(entity).remove::<EntityOnTiles>();
    }
}

#[cfg(test)]
mod test {
    use kloonorio_terrain::spawn_test_terrain;

    use super::*;
    use bevy::{
        app::Update,
        ecs::system::SystemState,
        math::IVec2,
        transform::{components::Transform, TransformPlugin},
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
                    Query<(Entity, &InitialTilePosition, &GlobalTransform, &EntityOnTiles)>,
                    Query<(&TileOccupants, &TilePos)>,
                    TerrainParams
                )> = SystemState::new(&mut app.world);
                let (entity_query, tile_query, terrain_params) = system_state.get_mut(&mut app.world);

                for (entity, initial_position, _entity_transform, entity_on_tiles) in entity_query.iter() {
                    let tile_entity = terrain_params.tile_entity_at_global_pos(initial_position.0.as_vec2()).unwrap();
                    let (tile_occupants, _tile_pos) = tile_query.get(tile_entity).unwrap();
                    assert!(tile_occupants.contains(&entity), "Entity should be in the tile occupants");
                    assert_eq!(entity_on_tiles.tile_entities().count(), 1, "Entity should be on one tile");
                }
            }

            // Move entities
            {
                let mut test_runner = TestRunner::default();
                let mut system_state: SystemState<(
                    Query<&mut Transform, With<TileTracked>>,
                )> = SystemState::new(&mut app.world);
                let mut transform_query = system_state.get_mut(&mut app.world).0;
                for mut transform in transform_query.iter_mut() {
                    let new_position = any_ivec2(-4, 4)
                        .prop_map(|pos| pos.as_vec2().extend(0.0))
                        .new_tree(&mut test_runner).unwrap().current();
                    transform.translation = new_position;
                }
                system_state.apply(&mut app.world);
            }

            app.update();

            // Check that entities are correctly assigned to tiles
            {
                let mut system_state: SystemState<(
                    Query<(Entity, &GlobalTransform, &EntityOnTiles)>,
                    Query<(&TileOccupants, &TilePos)>,
                    TerrainParams
                )> = SystemState::new(&mut app.world);
                let (entity_query, tile_query, terrain_params) = system_state.get_mut(&mut app.world);

                for (entity, entity_transform, entity_on_tiles) in entity_query.iter() {
                    let tile_entity = terrain_params.tile_entity_at_global_pos(entity_transform.translation().xy()).unwrap();
                    let (tile_occupants, _tile_pos) = tile_query.get(tile_entity).unwrap();
                    assert!(tile_occupants.contains(&entity), "Entity should be in the tile occupants");
                    assert_eq!(entity_on_tiles.tile_entities().count(), 1, "Entity should on one tile");
                }
            }
        }
    }

    proptest! {
        #[test]
        fn test_get_covered_tiles_for_collider_1x1(
            collider_positions in prop::collection::vec(
                any_ivec2(-4, 4),
                5
            ),
        ) {
            let collider = Collider::cuboid(0.4, 0.4);
            for pos in collider_positions {
                let covered_tiles = get_covered_tiles_for_collider(&collider, pos.as_vec2());
                assert_eq!(covered_tiles.len(), 1, "Collider should cover one tile");
                assert_eq!(covered_tiles[0], pos.as_vec2(), "Collider should cover the tile it is in");
            }
        }
    }

    proptest! {
        #[test]
        fn test_get_covered_tiles_for_collider_2x2(
            collider_positions in prop::collection::vec(
                any_ivec2(-4, 4),
                5
            ),
        ) {
            let collider = Collider::cuboid(0.9, 0.9);
            for pos in collider_positions {
                let collider_pos = pos.as_vec2() - Vec2::new(0.5, 0.5);
                let covered_tiles = get_covered_tiles_for_collider(&collider, collider_pos);
                assert_eq!(covered_tiles.len(), 4, "Collider should cover 4 tiles");
                let min_x = collider_pos.x.floor() as i32;
                let min_y = collider_pos.y.floor() as i32;
                let max_y = collider_pos.y.ceil() as i32;
                let max_x = collider_pos.x.ceil() as i32;
                for x in min_x..=max_x {
                    for y in min_y..=max_y {
                        let tile_pos = IVec2::new(x, y);
                        assert!(covered_tiles.contains(&tile_pos.as_vec2()), "Collider should cover the tile at {:?}", tile_pos);
                    }
                }
            }
        }
    }

    proptest! {
        #[test]
        fn test_get_covered_tiles_for_collider_3x3_tiles(
            collider_positions in prop::collection::vec(
                any_ivec2(-4, 4),
                5
            ),
        ) {
            let collider = Collider::cuboid(1.35, 1.35);
            for pos in collider_positions {
                let covered_tiles = get_covered_tiles_for_collider(&collider, pos.as_vec2());
                assert_eq!(covered_tiles.len(), 9, "Collider should cover 9 tiles");
                for x in -1..=1 {
                    for y in -1..=1 {
                        let tile_pos = pos + IVec2::new(x, y);
                        assert!(covered_tiles.contains(&tile_pos.as_vec2()), "Collider should cover the tile at {:?}", tile_pos);
                    }
                }
            }
        }
    }

    proptest! {
        #[test]
        fn test_update_entity_on_tile_system_collider(
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
                let collider = Collider::cuboid(0.4, 0.4);
                app.world.spawn((TileTracked, entity_transform, initial_position, collider));
            }

            app.update();

            app.add_systems(Update, update_entity_on_tile_system);
            app.update();

            // Check that entities are correctly assigned to tiles
            {
                let mut system_state: SystemState<(
                    Query<(Entity, &InitialTilePosition, &GlobalTransform, &EntityOnTiles)>,
                    Query<(&TileOccupants, &TilePos)>,
                    TerrainParams
                )> = SystemState::new(&mut app.world);
                let (entity_query, tile_query, terrain_params) = system_state.get_mut(&mut app.world);

                for (entity, initial_position, _entity_transform, entity_on_tiles) in entity_query.iter() {
                    let tile_entity = terrain_params.tile_entity_at_global_pos(initial_position.0.as_vec2()).unwrap();
                    let (tile_occupants, _tile_pos) = tile_query.get(tile_entity).unwrap();
                    assert!(tile_occupants.contains(&entity), "Entity should be in the tile occupants");
                    assert_eq!(entity_on_tiles.tile_entities().count(), 1, "Entity should be on one tile");
                }
            }
        }
    }
}
