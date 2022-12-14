use std::f32::consts::PI;

use bevy::{
    prelude::*,
    render::{Extract, RenderApp, RenderStage},
    sprite::{Anchor, ExtractedSprite, ExtractedSprites, SpriteSystem},
};

#[derive(Debug, Clone, Reflect)]
pub struct RotationAtlasIndexes(pub Vec<(f32, usize)>);

#[derive(Component, Debug, Clone, Reflect)]
pub struct IsometricSprite {
    pub color: Color,
    pub flip_x: bool,
    pub flip_y: bool,
    /// An optional custom size for the sprite that will be used when rendering, instead of the size
    /// of the sprite's image in the atlas
    pub custom_size: Option<Vec2>,
    pub anchor: Anchor,
    pub rotation_index: RotationAtlasIndexes,
}

impl Default for IsometricSprite {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            flip_x: false,
            flip_y: false,
            custom_size: None,
            anchor: Anchor::default(),
            rotation_index: RotationAtlasIndexes(vec![(0., 0)]),
        }
    }
}

#[derive(Bundle, Clone, Default)]
pub struct IsometricSpriteBundle {
    pub sprite: IsometricSprite,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub texture_atlas: Handle<TextureAtlas>,
    pub visibility: Visibility,
    pub computed_visibility: ComputedVisibility,
}

pub fn extract_isometric_sprites(
    mut extracted_sprites: ResMut<ExtractedSprites>,
    texture_atlases: Extract<Res<Assets<TextureAtlas>>>,
    isometric_sprite_query: Extract<
        Query<(
            Entity,
            &ComputedVisibility,
            &IsometricSprite,
            &GlobalTransform,
            &Handle<TextureAtlas>,
        )>,
    >,
) {
    // extracted_sprites.sprites.clear();
    for (entity, visibility, isometric_sprite, transform, texture_atlas_handle) in
        isometric_sprite_query.iter()
    {
        if !visibility.is_visible() {
            continue;
        }
        // PERF: we don't check in this function that the `Image` asset is ready, since it should be in most cases and hashing the handle is expensive
        let mut unrotated_transform = GlobalTransform::default();
        let compute_transform = transform.compute_transform();
        unrotated_transform =
            unrotated_transform.mul_transform(compute_transform.with_rotation(Quat::default()));

        let rotation = compute_transform.rotation;
        if let Some(texture_atlas) = texture_atlases.get(texture_atlas_handle) {
            let z_rotation = {
                let z_rotation = rotation.to_scaled_axis().z;
                if z_rotation < 0. {
                    z_rotation + PI * 2.
                } else {
                    z_rotation
                }
            };
            let index = isometric_sprite
                .rotation_index
                .0
                .iter()
                .rev()
                // shift the rotation so that z_rotation is always > 0
                .find(|(angle, _)| z_rotation + PI * 0.25 > *angle);

            if index.is_none() {
                error!("no index found for rotation {}", z_rotation);
                continue;
            }

            let index = index.unwrap().1;
            let rect = Some(texture_atlas.textures[index]);
            extracted_sprites.sprites.push(ExtractedSprite {
                entity,
                color: isometric_sprite.color,
                transform: unrotated_transform,
                rect,
                // Pass the custom size
                custom_size: isometric_sprite.custom_size,
                flip_x: isometric_sprite.flip_x,
                flip_y: isometric_sprite.flip_y,
                image_handle_id: texture_atlas.texture.id(),
                anchor: isometric_sprite.anchor.as_vec(),
            });
        }
    }
}

pub struct IsometricSpritePlugin;

impl Plugin for IsometricSpritePlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<IsometricSprite>();

        if let Ok(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_system_to_stage(
                RenderStage::Extract,
                extract_isometric_sprites.after(SpriteSystem::ExtractSprites),
            );
        }
    }
}
