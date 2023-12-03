use std::f32::consts::PI;

use bevy::{
    prelude::*,
    render::{Extract, RenderApp},
    sprite::{Anchor, ExtractedSprite, ExtractedSprites, SpriteSystem},
};
use serde::Deserialize;

use crate::discrete_rotation::DiscreteRotation;

#[derive(Debug, Clone, Deserialize, Reflect)]
pub struct RotationAtlasIndexes(pub Vec<(f32, usize)>);

impl Default for RotationAtlasIndexes {
    fn default() -> Self {
        Self(vec![(0., 0), (PI * 0.5, 1), (PI, 2), (PI * 1.5, 3)])
    }
}

#[derive(Component, Debug, Clone, Reflect)]
pub struct IsometricSprite {
    pub color: Color,
    pub flip_x: bool,
    pub flip_y: bool,
    /// An optional custom size for the sprite that will be used when rendering, instead of the size
    /// of the sprite's image in the atlas
    pub custom_size: Option<Vec2>,
    pub anchor: Anchor,
    pub sides: u32,
}

impl Default for IsometricSprite {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            flip_x: false,
            flip_y: false,
            custom_size: None,
            anchor: Anchor::default(),
            sides: 1,
        }
    }
}

#[derive(Bundle, Clone, Default)]
pub struct IsometricSpriteBundle {
    pub sprite: IsometricSprite,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub texture_atlas: Handle<TextureAtlas>,
    /// User indication of whether an entity is visible
    pub visibility: Visibility,
    /// Inherited visibility of an entity.
    pub inherited_visibility: InheritedVisibility,
    /// Algorithmically-computed indication of whether an entity is visible and should be extracted for rendering
    pub view_visibility: ViewVisibility,
}

pub fn extract_isometric_sprites(
    mut extracted_sprites: ResMut<ExtractedSprites>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    isometric_sprite_query: Extract<
        Query<(
            Entity,
            &IsometricSprite,
            &GlobalTransform,
            &DiscreteRotation,
            &ViewVisibility,
            &Handle<TextureAtlas>,
        )>,
    >,
) {
    // extracted_sprites.sprites.clear();
    for (
        entity,
        isometric_sprite,
        transform,
        discrete_rotation,
        view_visibility,
        texture_atlas_handle,
    ) in isometric_sprite_query.iter()
    {
        if !view_visibility.get() {
            continue;
        }
        // PERF: we don't check in this function that the `Image` asset is ready, since it should be in most cases and hashing the handle is expensive
        let mut unrotated_transform = GlobalTransform::default();
        let compute_transform = transform.compute_transform();
        unrotated_transform =
            unrotated_transform.mul_transform(compute_transform.with_rotation(Quat::default()));

        if let Some(texture_atlas) = texture_atlases.get(texture_atlas_handle) {
            let index = discrete_rotation.get();

            let rect = Some(texture_atlas.textures[index]);
            extracted_sprites.sprites.insert(
                entity,
                ExtractedSprite {
                    color: isometric_sprite.color,
                    transform: unrotated_transform,
                    rect,
                    // Pass the custom size
                    custom_size: isometric_sprite.custom_size,
                    flip_x: isometric_sprite.flip_x,
                    flip_y: isometric_sprite.flip_y,
                    image_handle_id: texture_atlas.texture.id(),
                    anchor: isometric_sprite.anchor.as_vec(),
                    original_entity: None,
                },
            );
        }
    }
}

pub struct IsometricSpritePlugin;

impl Plugin for IsometricSpritePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<IsometricSprite>();

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                ExtractSchedule,
                extract_isometric_sprites.after(SpriteSystem::ExtractSprites),
            );
        }
    }
}
