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
    structure_components::transport_belt::TransportBelt,
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
) {
    for (transport_belt_entity, _transport_belt_builder, transform) in &transport_belt_builder_query
    {
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

        let belt = TransportBelt::new(next_belt_entity);
        commands
            .entity(transport_belt_entity)
            .insert(belt)
            .remove::<TransportBeltBuilder>();

        // Find belts to the north, east, south, and west of this belt
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
                    let mut belt = transport_belt_query.get_mut(other_belt_entity).unwrap();
                    belt.next_belt = Some(transport_belt_entity);
                }
            }
        }
    }
}
