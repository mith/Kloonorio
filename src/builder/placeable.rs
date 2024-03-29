use bevy::{ecs::system::EntityCommands, math::Vec3Swizzles, prelude::*, utils::HashSet};
use bevy_rapier2d::prelude::*;
use kloonorio_core::{
    discrete_rotation::DiscreteRotation,
    inventory::{Fuel, Inventory, Output, Source, Storage},
    item::Item,
    structure::{Structure, Structures},
    structure_components::{
        assembler::Assembler, burner::Burner, smelter::Smelter, StructureComponent,
    },
    types::{Building, CraftingQueue, Ghost},
};
use kloonorio_render::isometric_sprite::{IsometricSprite, IsometricSpriteBundle};
use kloonorio_terrain::{CursorWorldPos, TILE_SIZE};
use kloonorio_ui::{inventory_grid::Hand, picker::Pickable, HoveringUI};

use crate::{
    builder::{
        inserter_builder::InserterBuilder, miner_builder::MinerBuilder,
        transport_belt_builder::TransportBeltBuilder,
    },
    entity_tile_tracking::TileTracked,
    ysort::YSort,
};

pub fn placeable(
    mut commands: Commands,
    mut placeable_query: Query<(Entity, &mut Hand), Without<HoveringUI>>,
    cursor_pos: Res<CursorWorldPos>,
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
            let structure_name = &*stack.item;
            let Some(structure) = structures.get(structure_name) else {
                continue;
            };
            let texture_atlas_handle =
                create_structure_texture_atlas(&asset_server, structure, &mut texture_atlases);

            let translation = cursor_to_structure_position(&cursor_pos, structure);

            let rotation = *hand
                .rotation
                .get_or_insert_with(|| DiscreteRotation::new(structure.sides.try_into().unwrap()));

            if rapier_context
                .intersection_with_shape(
                    translation,
                    0.,
                    &structure_collider(structure),
                    QueryFilter::new().exclude_sensors(),
                )
                .is_some()
            {
                spawn_structure_ghost(
                    &mut commands,
                    translation,
                    rotation,
                    texture_atlas_handle,
                    Color::rgba(1.0, 0.3, 0.3, 0.5),
                    structure,
                );
            } else if mouse_input.just_pressed(MouseButton::Left) {
                if inventory.remove_items(&[(Item::new(structure.name.clone()), 1)]) {
                    debug!("Placing {:?}", structure);
                    place_structure(
                        &mut commands,
                        texture_atlas_handle.clone(),
                        translation,
                        rotation,
                        structure,
                    );
                    if !inventory.has_items(&[(Item::new(structure.name.clone()), 1)]) {
                        hand.clear();
                    }
                }
            } else {
                spawn_structure_ghost(
                    &mut commands,
                    translation,
                    rotation,
                    texture_atlas_handle,
                    Color::rgba(1.0, 1.0, 1.0, 0.5),
                    structure,
                );
            }
        }
    }
}

pub fn placeable_rotation(
    keys: Res<Input<KeyCode>>,
    mut placeable_query: Query<(Entity, &mut Hand), Without<HoveringUI>>,
) {
    if keys.just_pressed(KeyCode::R) {
        if let Ok((_hand_entity, mut hand)) = placeable_query.get_single_mut() {
            if let Some(rotation) = hand.rotation.as_mut() {
                rotation.rotate();
                debug!("Rotated to {:?}", hand.rotation);
            }
        }
    }
}

fn cursor_to_structure_position(cursor_pos: &CursorWorldPos, structure: &Structure) -> Vec2 {
    let min_corner: Vec2 = cursor_pos.0.xy() - (structure.size.as_vec2() / 2.0);
    let grid_fitted_min_corner = min_corner.ceil();
    let structure_rect = Rect::from_corners(
        grid_fitted_min_corner,
        grid_fitted_min_corner + structure.size.as_vec2(),
    );

    structure_rect.center() - Vec2::splat(0.5)
}

pub fn create_structure_texture_atlas(
    asset_server: &Res<AssetServer>,
    structure: &Structure,
    texture_atlases: &mut ResMut<Assets<TextureAtlas>>,
) -> Handle<TextureAtlas> {
    let texture_handle = asset_server.load(format!(
        "textures/{}.png",
        &structure.name.to_lowercase().replace(' ', "_")
    ));
    let (rows, columns) = {
        if structure.animated {
            (10, 12)
        } else {
            (1, structure.sides as usize)
        }
    };
    let tile_size = structure_texture_size(structure);
    let texture_atlas =
        TextureAtlas::from_grid(texture_handle, tile_size, columns, rows, None, None);

    texture_atlases.add(texture_atlas)
}

fn structure_texture_size(structure: &Structure) -> Vec2 {
    if structure.animated {
        // For animated structures, add 2 tiles to each dimension for the border
        (structure.size.as_vec2() + Vec2::new(2., 2.)) * Vec2::new(TILE_SIZE.x, TILE_SIZE.y)
    } else {
        // For non-animated structures, just multiply the structure size by the tile size
        structure.size.as_vec2() * Vec2::new(TILE_SIZE.x, TILE_SIZE.y)
    }
}

pub fn place_structure(
    commands: &mut Commands,
    texture_atlas_handle: Handle<TextureAtlas>,
    translation: Vec2,
    rotation: DiscreteRotation,
    structure: &Structure,
) {
    let mut structure_entity = spawn_structure_base(
        commands,
        structure.name.to_string(),
        rotation,
        translation,
        structure,
        texture_atlas_handle,
        Color::WHITE,
    );
    structure_entity.insert((
        Building,
        Pickable,
        TileTracked,
        structure_collider(structure),
    ));

    spawn_structure_components(&mut structure_entity, structure);
}

fn structure_sprite_size(structure: &Structure) -> Vec2 {
    if structure.animated {
        structure.size.as_vec2() + 2.
    } else {
        structure.size.as_vec2()
    }
}

fn structure_collider(structure: &Structure) -> Collider {
    Collider::cuboid(structure.collider.x * 0.5, structure.collider.y * 0.5)
}

pub fn spawn_structure_ghost(
    commands: &mut Commands,
    translation: Vec2,
    rotation: DiscreteRotation,
    texture_atlas_handle: Handle<TextureAtlas>,
    color: Color,
    structure: &Structure,
) {
    spawn_structure_base(
        commands,
        structure.name.to_string() + " (ghost)",
        rotation,
        translation,
        structure,
        texture_atlas_handle,
        color,
    )
    .insert(Ghost);
}

fn spawn_structure_base<'w, 's, 'a>(
    commands: &'a mut Commands<'w, 's>,
    name: String,
    rotation: DiscreteRotation,
    translation: Vec2,
    structure: &Structure,
    texture_atlas_handle: Handle<TextureAtlas>,
    color: Color,
) -> EntityCommands<'w, 's, 'a> {
    let structure_entity = commands.spawn((
        Name::new(name),
        rotation,
        YSort { base_layer: 1. },
        IsometricSpriteBundle {
            texture_atlas: texture_atlas_handle,
            transform: Transform::from_translation(translation.extend(1.))
                .with_rotation(Quat::from_rotation_z(-rotation.radians())),

            sprite: IsometricSprite {
                color,
                sides: structure.sides,
                custom_size: Some(structure_sprite_size(structure)),
                ..default()
            },
            ..default()
        },
    ));

    structure_entity
}

pub fn spawn_structure_components(entity_commands: &mut EntityCommands, structure: &Structure) {
    let span = info_span!("spawn_components", structure = ?structure.name);
    let _enter = span.enter();
    for component in &structure.components {
        match component {
            StructureComponent::Smelter => {
                debug!("Spawning smelter");
                entity_commands.insert(Smelter);
            }
            StructureComponent::Burner => {
                debug!("Spawning burner");
                entity_commands.insert(Burner::default());
            }
            StructureComponent::CraftingQueue => {
                debug!("Spawning crafting queue");
                entity_commands.insert(CraftingQueue::default());
            }
            StructureComponent::Inventory(slots) => {
                debug!("Spawning inventory");
                entity_commands.with_children(|p| {
                    p.spawn((
                        Storage,
                        Inventory::new(*slots),
                        TransformBundle::default(),
                        Sensor,
                        structure_collider(structure),
                        TileTracked,
                    ));
                });
            }
            StructureComponent::Source(slots, filter) => {
                debug!("Spawning source");
                entity_commands.with_children(|p| {
                    p.spawn((
                        Source,
                        Inventory::new_with_filter(
                            *slots,
                            filter.iter().map(|s| Item::new(s.clone())).collect(),
                        ),
                        TransformBundle::default(),
                        Sensor,
                        structure_collider(structure),
                        TileTracked,
                    ));
                });
            }
            StructureComponent::Output(slots) => {
                debug!("Spawning output");
                entity_commands.with_children(|p| {
                    p.spawn((
                        Output,
                        Inventory::new(*slots),
                        TransformBundle::default(),
                        Sensor,
                        structure_collider(structure),
                        TileTracked,
                    ));
                });
            }
            StructureComponent::Fuel(slots) => {
                debug!("Spawning fuel");
                entity_commands.with_children(|p| {
                    p.spawn((
                        Fuel,
                        Inventory::new_with_filter(*slots, HashSet::from_iter([Item::new("Coal")])),
                        TransformBundle::default(),
                        Sensor,
                        structure_collider(structure),
                        TileTracked,
                    ));
                });
            }
            StructureComponent::Miner(speed) => {
                debug!("Spawning miner");
                entity_commands.insert(MinerBuilder::new(*speed));
            }
            StructureComponent::Inserter(speed, capacity) => {
                debug!("Spawning inserter");
                entity_commands.insert(InserterBuilder::new(*speed, *capacity));
            }
            StructureComponent::TransportBelt => {
                debug!("Spawning transport belt");
                entity_commands.insert(TransportBeltBuilder);
            }
            StructureComponent::Assembler => {
                debug!("Spawning assembler");
                entity_commands.insert(Assembler::default());
            }
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn cursor_to_structure_position_zero() {
        let cursor_pos = CursorWorldPos(Vec3::ZERO);
        let structure = Structure {
            name: "test".into(),
            size: IVec2::new(1, 1),
            sides: 1,
            collider: Vec2::new(1., 1.),
            components: vec![],
            animated: false,
        };

        let result = cursor_to_structure_position(&cursor_pos, &structure);

        assert_eq!(result, Vec2::ZERO);
    }

    #[test]
    fn structure_texture_size_1x1() {
        let structure = Structure {
            name: "test".into(),
            size: IVec2::new(1, 1),
            sides: 1,
            collider: Vec2::new(1., 1.),
            components: vec![],
            animated: true,
        };

        let result = structure_texture_size(&structure);

        assert_eq!(result, Vec2::new(48., 48.));
    }

    #[test]
    fn structure_texture_size_2x3() {
        let structure = Structure {
            name: "test".into(),
            size: IVec2::new(2, 3),
            sides: 1,
            collider: Vec2::new(1., 1.),
            components: vec![],
            animated: true,
        };

        let result = structure_texture_size(&structure);

        assert_eq!(result, Vec2::new(64., 80.));
    }

    #[test]
    fn structure_texture_size_3x3() {
        let structure = Structure {
            name: "test".into(),
            size: IVec2::new(3, 3),
            sides: 1,
            collider: Vec2::new(1., 1.),
            components: vec![],
            animated: true,
        };

        let result = structure_texture_size(&structure);

        assert_eq!(result, Vec2::new(80., 80.));
    }

    #[test]
    fn structure_sprite_size_1x1() {
        let structure = Structure {
            name: "test".into(),
            size: IVec2::new(1, 1),
            sides: 1,
            collider: Vec2::new(1., 1.),
            components: vec![],
            animated: true,
        };

        let result = structure_sprite_size(&structure);

        assert_eq!(result, Vec2::new(3., 3.));
    }

    #[test]
    fn structure_sprite_size_3x3() {
        let structure = Structure {
            name: "test".into(),
            size: IVec2::new(3, 3),
            sides: 1,
            collider: Vec2::new(1., 1.),
            components: vec![],
            animated: true,
        };

        let result = structure_sprite_size(&structure);

        assert_eq!(result, Vec2::new(5., 5.));
    }
}
