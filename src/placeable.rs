use std::f32::consts::PI;

use bevy::prelude::*;
use bevy::{math::Vec3Swizzles, utils::HashSet};
use bevy_rapier2d::prelude::*;

use crate::isometric_sprite::{self, IsometricSprite, IsometricSpriteBundle, RotationAtlasIndexes};
use crate::types::Rotation;
use crate::{
    burner::Burner,
    inserter::Inserter,
    inventory::{Fuel, Inventory, Output, Source},
    inventory_grid::Hand,
    loading::Structures,
    miner::Miner,
    smelter::Smelter,
    structure_loader::{Structure, StructureComponent},
    terrain::{CursorPos, TILE_SIZE},
    types::{CraftingQueue, Product},
    HoveringUI,
};

#[derive(Component)]
pub struct Ghost;

#[derive(Component)]
pub struct Building;

#[derive(Component)]
pub struct Contains(Vec<Entity>);

pub fn placeable(
    mut commands: Commands,
    mut placeable_query: Query<(Entity, &mut Hand), Without<HoveringUI>>,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ghosts: Query<Entity, With<Ghost>>,
    asset_server: Res<AssetServer>,
    rapier_context: Res<RapierContext>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    structures: Res<Structures>,
    mut inventories_query: Query<&mut Inventory>,
) {
    let span = info_span!("Placeable");
    let _enter = span.enter();
    // delete old ghosts
    for ghost in ghosts.iter() {
        commands.entity(ghost).despawn_recursive();
    }

    for (hand_entity, mut hand) in &mut placeable_query {
        let mut inventory = inventories_query.get_mut(hand_entity).unwrap();
        if let Some(Some(stack)) = hand.get_item().map(|ih| inventory.slots[ih.slot].clone()) {
            if let Product::Structure(structure_name) = &stack.resource {
                let structure = structures.get(structure_name).unwrap();
                let texture_atlas_handle =
                    create_structure_texture_atlas(&asset_server, structure, &mut texture_atlases);

                let translation = cursor_to_structure_position(&cursor_pos, structure);
                let rotation = hand.rotation.as_ref().unwrap_or(&Rotation(0.)).clone();

                let transform = Transform::from_translation(translation.extend(1.0))
                    .with_rotation(Quat::from_rotation_z(rotation.0));

                if rapier_context
                    .intersection_with_shape(
                        translation,
                        0.,
                        &structure_collider(structure),
                        QueryFilter::new(),
                    )
                    .is_some()
                {
                    spawn_ghost(
                        &mut commands,
                        transform,
                        texture_atlas_handle,
                        Color::rgba(1.0, 0.3, 0.3, 0.5),
                    );
                } else if mouse_input.just_pressed(MouseButton::Left) {
                    if inventory.remove_items(&[(Product::Structure(structure.name.clone()), 1)]) {
                        info!("Placing {:?}", structure);
                        place_structure(
                            &mut commands,
                            texture_atlas_handle.clone(),
                            transform,
                            structure,
                        );
                        if !inventory.has_items(&[(Product::Structure(structure.name.clone()), 1)])
                        {
                            hand.clear();
                        }
                    }
                } else {
                    spawn_ghost(
                        &mut commands,
                        transform,
                        texture_atlas_handle,
                        Color::rgba(1.0, 1.0, 1.0, 0.5),
                    );
                }
            }
        }
    }
}

pub fn placeable_rotation(
    keys: Res<Input<KeyCode>>,
    mut placeable_query: Query<&mut Hand, Without<HoveringUI>>,
) {
    if keys.just_pressed(KeyCode::R) {
        if let Ok(mut hand) = placeable_query.get_single_mut() {
            hand.rotation = Some(Rotation(
                (hand.rotation.as_ref().unwrap_or(&Rotation(0.)).0 + (PI * 0.5)) % (PI * 2.),
            ));

            info!("Rotated to {:?}", hand.rotation);
        }
    }
}

fn cursor_to_structure_position(cursor_pos: &Res<CursorPos>, structure: &Structure) -> Vec2 {
    let tile_size_v = Vec2::new(TILE_SIZE.x, TILE_SIZE.y);
    let min_corner: Vec2 = cursor_pos.0.xy() - (structure.size.as_vec2() / 2.0 * tile_size_v);
    let grid_fitted_min_corner = (min_corner / tile_size_v).ceil() * tile_size_v;
    let structure_rect = Rect::from_corners(
        grid_fitted_min_corner,
        grid_fitted_min_corner + structure.size.as_vec2() * tile_size_v,
    );
    let translation = structure_rect.center() - tile_size_v / 2.0;
    translation
}

pub fn create_structure_texture_atlas(
    asset_server: &Res<AssetServer>,
    structure: &Structure,
    texture_atlases: &mut ResMut<Assets<TextureAtlas>>,
) -> Handle<TextureAtlas> {
    let texture_handle = asset_server.load(&format!(
        "textures/{}.png",
        &structure.name.to_lowercase().replace(" ", "_")
    ));
    let texture_atlas = TextureAtlas::from_grid(
        texture_handle,
        structure.size.as_vec2() * Vec2::new(TILE_SIZE.x, TILE_SIZE.y),
        4,
        1,
        None,
        None,
    );
    let texture_atlas_handle = texture_atlases.add(texture_atlas);
    texture_atlas_handle
}

pub fn place_structure(
    commands: &mut Commands,
    texture_atlas_handle: Handle<TextureAtlas>,
    transform: Transform,
    structure: &Structure,
) {
    let atlas = RotationAtlasIndexes(vec![(0., 0), (PI * 0.5, 1), (PI, 2), (PI * 1.5, 3)]);
    let structure_entity = commands
        .spawn((
            IsometricSpriteBundle {
                texture_atlas: texture_atlas_handle,
                transform,
                sprite: IsometricSprite {
                    rotation_index: atlas,
                    ..default()
                },
                ..default()
            },
            structure_collider(structure),
            Building,
            Name::new(structure.name.to_string()),
        ))
        .id();
    spawn_components(commands, structure, structure_entity);
}

fn structure_collider(structure: &Structure) -> Collider {
    Collider::cuboid(
        structure.collider.x * 0.5 * TILE_SIZE.x,
        structure.collider.y * 0.5 * TILE_SIZE.y,
    )
}

pub fn spawn_ghost(
    commands: &mut Commands,
    transform: Transform,
    texture_atlas_handle: Handle<TextureAtlas>,
    color: Color,
) {
    let atlas = RotationAtlasIndexes(vec![(0., 0), (PI * 0.5, 1), (PI, 2), (PI * 1.5, 3)]);
    commands.spawn((
        IsometricSpriteBundle {
            transform,
            texture_atlas: texture_atlas_handle,
            sprite: IsometricSprite {
                color,
                rotation_index: atlas,
                ..default()
            },
            ..default()
        },
        Ghost,
    ));
}

pub fn spawn_components(commands: &mut Commands, structure: &Structure, structure_entity: Entity) {
    for component in &structure.components {
        match component {
            StructureComponent::Smelter => {
                commands.entity(structure_entity).insert(Smelter);
            }
            StructureComponent::Burner => {
                commands.entity(structure_entity).insert(Burner::new());
            }
            StructureComponent::CraftingQueue => {
                commands
                    .entity(structure_entity)
                    .insert(CraftingQueue::default());
            }
            StructureComponent::Inventory(slots) => {
                commands
                    .entity(structure_entity)
                    .insert(Inventory::new(*slots));
            }
            StructureComponent::Source(slots, filter) => {
                commands.entity(structure_entity).with_children(|p| {
                    p.spawn((Source, Inventory::new_with_filter(*slots, filter.clone())));
                });
            }
            StructureComponent::Output(slots) => {
                commands.entity(structure_entity).with_children(|p| {
                    p.spawn((Output, Inventory::new(*slots)));
                });
            }
            StructureComponent::Fuel(slots) => {
                commands.entity(structure_entity).with_children(|p| {
                    p.spawn((
                        Fuel,
                        Inventory::new_with_filter(
                            *slots,
                            HashSet::from_iter([Product::Intermediate("Coal".into())]),
                        ),
                    ));
                });
            }
            StructureComponent::Miner(speed) => {
                commands.entity(structure_entity).insert(Miner::new(*speed));
            }
            StructureComponent::Inserter(speed, capacity) => {
                commands
                    .entity(structure_entity)
                    .insert(Inserter::new(*speed, *capacity));
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn placeable_rotation_one_rotation() {
        let mut app = App::new();

        let mut placeable = Hand::default();

        let hand_id = app.world.spawn(placeable.clone()).id();

        let mut input = Input::<KeyCode>::default();
        input.press(KeyCode::R);
        app.world.insert_resource(input);

        app.add_system(placeable_rotation);
        app.update();

        assert_eq!(
            app.world
                .get::<Hand>(hand_id)
                .unwrap()
                .rotation
                .as_ref()
                .unwrap()
                .0,
            PI * 0.5
        );
    }
}
