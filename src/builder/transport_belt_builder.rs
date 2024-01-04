use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query},
    },
    math::{Vec3, Vec3Swizzles},
    reflect::Reflect,
    transform::components::GlobalTransform,
};
use kloonorio_core::{
    structure_components::transport_belt::TransportBelt, tile_occupants::TileOccupants,
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
    terrain_params: TerrainParams,
) {
    for (transport_belt_entity, _transport_belt_builder, transform) in &transport_belt_builder_query
    {
        let next_tile_pos = transform.transform_point(Vec3::new(0., 1., 0.));
        let next_tile = terrain_params.tile_entity_at_global_pos(next_tile_pos.xy());
        let next_tile_belt_entity = next_tile
            .and_then(|tile| tile_occupants_query.get(tile).ok())
            .and_then(|tile_occupants| {
                tile_occupants
                    .iter()
                    .find(|occupant| transport_belt_query.contains(**occupant))
                    .copied()
            });

        let belt = TransportBelt::new(next_tile_belt_entity);
        commands
            .entity(transport_belt_entity)
            .insert(belt)
            .remove::<TransportBeltBuilder>();

        // Check if there is a belt in the previous tile and update it to point to this belt
        let prev_tile_pos = transform.transform_point(Vec3::new(0., -1., 0.));
        let prev_tile = terrain_params.tile_entity_at_global_pos(prev_tile_pos.xy());
        if let Some(prev_tile) = prev_tile {
            if let Ok(prev_tile_occupants) = tile_occupants_query.get(prev_tile) {
                if let Some(prev_tile_belt_entity) = prev_tile_occupants
                    .iter()
                    .find(|occupant| transport_belt_query.contains(**occupant))
                {
                    let mut prev_tile_belt = transport_belt_query
                        .get_mut(*prev_tile_belt_entity)
                        .unwrap();
                    prev_tile_belt.next_belt = Some(transport_belt_entity);
                }
            }
        }
    }
}
