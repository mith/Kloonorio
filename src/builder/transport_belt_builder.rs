use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query},
    },
    math::{Vec2, Vec3, Vec3Swizzles},
    reflect::Reflect,
    transform::components::GlobalTransform,
};
use kloonorio_core::{
    discrete_rotation::{CompassDirection, DiscreteRotation},
    structure_components::transport_belt::{NextBelt, PreviousBelts, TransportBelt},
    tile_occupants::TileOccupants,
    types::AppState,
};
use kloonorio_terrain::TerrainParams;

pub struct TransportBeltBuilderPlugin;

impl Plugin for TransportBeltBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TransportBeltBuilder>().add_systems(
            Update,
            build_transport_belt.run_if(in_state(AppState::Running)),
        );
    }
}

#[derive(Component, Debug, Reflect)]
pub struct TransportBeltBuilder;

fn build_transport_belt(
    mut commands: Commands,
    transport_belt_builder_query: Query<(Entity, &TransportBeltBuilder, &GlobalTransform)>,
    mut transport_belt_query: Query<&mut TransportBelt>,
    tile_occupants_query: Query<&TileOccupants>,
    discrete_rotation_query: Query<&DiscreteRotation>,
    terrain_params: TerrainParams,
    mut previous_belts_query: Query<&mut PreviousBelts>,
) {
    for (transport_belt_entity, _transport_belt_builder, transform) in &transport_belt_builder_query
    {
        // Find the next belt in the direction this belt is facing
        let next_tile_pos = transform.transform_point(Vec3::new(0., 1., 0.));
        let next_belt_entity = terrain_params
            .tile_entity_at_global_pos(next_tile_pos.xy())
            .and_then(|tile| tile_occupants_query.get(tile).ok())
            .and_then(|tile_occupants| {
                tile_occupants
                    .iter()
                    .find(|occupant| transport_belt_query.contains(**occupant))
                    .copied()
            });

        if let Some(next_belt_entity) = next_belt_entity {
            // Add the next belt component
            commands
                .entity(transport_belt_entity)
                .insert(NextBelt(next_belt_entity));

            // Add this belt to the list of previous belts of the next belt
            let mut previous_belts = previous_belts_query
                .get_mut(next_belt_entity)
                .unwrap_or_else(|_| {
                    panic!(
                        "Expected {:?} to have PreviousBelts component",
                        next_belt_entity
                    )
                });
            previous_belts.belts.insert(transport_belt_entity);
        }

        // Find belts to the north, east, south, and west of this belt
        let mut previous_belts = Vec::new();

        let north_tile_pos = transform.translation().xy() + Vec2::new(0., 1.);
        let dir_from_north = CompassDirection::South;
        let east_tile_pos = transform.translation().xy() + Vec2::new(1., 0.);
        let dir_from_east = CompassDirection::West;
        let south_tile_pos = transform.translation().xy() + Vec2::new(0., -1.);
        let dir_from_south = CompassDirection::North;
        let west_tile_pos = transform.translation().xy() + Vec2::new(-1., 0.);
        let dir_from_west = CompassDirection::East;
        for (tile_pos, dir_from) in &[
            (north_tile_pos, dir_from_north),
            (east_tile_pos, dir_from_east),
            (south_tile_pos, dir_from_south),
            (west_tile_pos, dir_from_west),
        ] {
            if let Some(other_belt_entity) = terrain_params
                .tile_entity_at_global_pos(*tile_pos)
                .and_then(|tile| tile_occupants_query.get(tile).ok())
                .and_then(|tile_occupants| {
                    tile_occupants
                        .iter()
                        .find(|occupant| transport_belt_query.contains(**occupant))
                        .copied()
                })
            {
                let other_belt_dir = discrete_rotation_query
                    .get(other_belt_entity)
                    .unwrap()
                    .compass_direction();

                if other_belt_dir == *dir_from {
                    // Other belt is depositing items onto this belt

                    // Mark this belt as the next belt of the other belt
                    commands
                        .entity(other_belt_entity)
                        .insert(NextBelt(transport_belt_entity));

                    // Add the other belt as a previous belt of this belt
                    previous_belts.push(other_belt_entity);
                }
            }
        }

        let belt = TransportBelt::default();
        commands
            .entity(transport_belt_entity)
            .insert((
                belt,
                PreviousBelts {
                    belts: previous_belts.iter().copied().collect(),
                },
            ))
            .remove::<TransportBeltBuilder>();
    }
}
