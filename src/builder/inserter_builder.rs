use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query, Res},
    },
    hierarchy::BuildChildren,
    math::{Vec2, Vec3},
    prelude::default,
    transform::components::{GlobalTransform, Transform},
};

use kloonorio_core::{
    discrete_rotation::{DiscreteRotation, SideCount},
    structure_components::inserter::{
        inserter_dropoff_location, inserter_pickup_location, Inserter, InserterHand,
    },
    types::AppState,
};
use kloonorio_render::{
    isometric_sprite::{IsometricSprite, IsometricSpriteBundle},
    item_textures::ItemTextures,
};
use kloonorio_terrain::TerrainParams;
use tracing::info_span;

pub struct InserterBuilderPlugin;

impl Plugin for InserterBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, build_inserter.run_if(in_state(AppState::Running)));
    }
}

#[derive(Component)]
pub struct InserterBuilder {
    speed: f32,
    capacity: u32,
}
impl InserterBuilder {
    pub fn new(speed: f32, capacity: u32) -> Self {
        InserterBuilder { speed, capacity }
    }
}

fn build_inserter(
    mut commands: Commands,
    inserter_builder_query: Query<(Entity, &InserterBuilder, &GlobalTransform)>,
    terrain_params: TerrainParams,
    item_textures: Res<ItemTextures>,
) {
    for (inserter_entity, inserter_builder, transform) in &mut inserter_builder_query.iter() {
        let span = info_span!("Build inserter", inserter = ?inserter_entity);
        let _enter = span.enter();

        let pickup_tile_location = inserter_pickup_location(transform);
        let dropoff_tile_location = inserter_dropoff_location(transform);

        let pickup_tile_entity = terrain_params
            .tile_entity_at_global_pos(pickup_tile_location)
            .unwrap();
        let dropoff_tile_entity = terrain_params
            .tile_entity_at_global_pos(dropoff_tile_location)
            .unwrap();

        let inserter_hand_entity = commands
            .spawn((
                IsometricSpriteBundle {
                    transform: Transform::from_translation(Vec3::new(0., 0., 0.5)),
                    texture_atlas: item_textures.get_texture_atlas_handle(),
                    sprite: IsometricSprite {
                        sides: 1,
                        custom_size: Some(Vec2::new(0.3, 0.3)),
                        ..default()
                    },
                    ..default()
                },
                DiscreteRotation::new(SideCount::One),
                // YSort { base_layer: 1. },
            ))
            .id();

        let inserter = Inserter::new(
            inserter_builder.speed,
            inserter_builder.capacity,
            pickup_tile_entity,
            dropoff_tile_entity,
        );
        commands
            .entity(inserter_entity)
            .add_child(inserter_hand_entity)
            .insert((inserter, InserterHand(inserter_hand_entity)))
            .remove::<InserterBuilder>();
    }
}
