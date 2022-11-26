use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

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
    mut placeable_query: Query<(&mut Inventory, &mut Hand), Without<HoveringUI>>,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ghosts: Query<Entity, With<Ghost>>,
    asset_server: Res<AssetServer>,
    rapier_context: Res<RapierContext>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    structures: Res<Structures>,
) {
    // delete old ghosts
    for ghost in ghosts.iter() {
        commands.entity(ghost).despawn_recursive();
    }

    for (mut inventory, mut hand) in &mut placeable_query {
        // TODO: make this simpler
        if let Some(Some(stack)) = hand.get_item().map(|ih| inventory.slots[ih.slot].clone()) {
            if let Product::Structure(structure_name) = &stack.resource {
                let structure = structures.get(structure_name).unwrap();
                let texture_atlas_handle =
                    create_structure_texture_atlas(&asset_server, structure, &mut texture_atlases);

                let translation = cursor_to_structure_position(&cursor_pos, structure);

                let transform = Transform::from_translation(translation.extend(1.0));

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
        2,
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
    let structure_entity = commands
        .spawn((
            SpriteSheetBundle {
                texture_atlas: texture_atlas_handle,
                transform,
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
    commands.spawn((
        SpriteSheetBundle {
            transform,
            texture_atlas: texture_atlas_handle,
            sprite: TextureAtlasSprite { color, ..default() },
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
            StructureComponent::Source(slots) => {
                commands.entity(structure_entity).with_children(|p| {
                    p.spawn((Source, Inventory::new(*slots)));
                });
            }
            StructureComponent::Output(slots) => {
                commands.entity(structure_entity).with_children(|p| {
                    p.spawn((Output, Inventory::new(*slots)));
                });
            }
            StructureComponent::Fuel(slots) => {
                commands.entity(structure_entity).with_children(|p| {
                    p.spawn((Fuel, Inventory::new(*slots)));
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
