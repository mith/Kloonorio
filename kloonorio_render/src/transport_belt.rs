use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        entity::Entity,
        query::With,
        schedule::{common_conditions::in_state, IntoSystemConfigs, SystemSet},
        system::{Commands, Query, Res},
    },
    hierarchy::{BuildChildren, DespawnRecursiveExt},
    math::Vec2,
    prelude::default,
    sprite::{SpriteSheetBundle, TextureAtlasSprite},
    transform::components::Transform,
};
use kloonorio_core::{
    structure_components::transport_belt::{BeltItem, TransportBelt},
    types::AppState,
};

use crate::item_textures::ItemTextures;

pub fn create_transport_belt_sprites(
    mut commands: Commands,
    transport_belt_query: Query<(Entity, &TransportBelt)>,
    belt_item_query: Query<Entity, With<BeltItem>>,
    item_textures: Res<ItemTextures>,
) {
    for belt_item in belt_item_query.iter() {
        commands.entity(belt_item).despawn_recursive();
    }

    for (transport_belt_entity, transport_belt) in transport_belt_query.iter() {
        for (i, slot) in transport_belt.slots().enumerate() {
            if let Some(product) = slot {
                let sprite_transform = Transform::from_xyz(0., (i as i32 - 1) as f32 * 0.3, 1.);
                let slot_sprite = commands
                    .spawn((
                        BeltItem,
                        SpriteSheetBundle {
                            transform: sprite_transform,
                            texture_atlas: item_textures.get_texture_atlas_handle(),
                            sprite: TextureAtlasSprite {
                                // Pass the custom size
                                custom_size: Some(Vec2::new(0.4, 0.4)),
                                index: item_textures.get_texture_index(product).unwrap(),
                                ..default()
                            },
                            ..default()
                        },
                    ))
                    .id();
                commands
                    .entity(transport_belt_entity)
                    .add_child(slot_sprite);
            }
        }
    }
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
struct BeltItemRenderSet;

pub struct TransportBeltRenderPlugin;

impl Plugin for TransportBeltRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            create_transport_belt_sprites
                .run_if(in_state(AppState::Running))
                .in_set(BeltItemRenderSet),
        );
    }
}
