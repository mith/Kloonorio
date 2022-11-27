use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_ecs_tilemap::tiles::TileTextureIndex;
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};

use crate::{
    inventory::{Fuel, Inventory, Output, Source, Stack},
    terrain::{COAL, IRON, STONE, TREE},
    types::Product,
};

pub fn texture_id_to_product(index: TileTextureIndex) -> Product {
    match index.0 {
        COAL => Product::Intermediate("Coal".into()),
        IRON => Product::Intermediate("Iron ore".into()),
        STONE => Product::Intermediate("Stone".into()),
        TREE => Product::Intermediate("Wood".into()),
        _ => panic!("Unknown product"),
    }
}

pub fn product_to_texture(product: &Product) -> String {
    match product {
        Product::Intermediate(name) => name.to_lowercase().replace(" ", "_"),
        _ => "no_icon".to_string(),
    }
}

/// Spawn a stack of items at the given position
pub fn spawn_stack(
    commands: &mut Commands,
    stack: Stack,
    asset_server: &Res<AssetServer>,
    position: Vec3,
) {
    let path = format!("textures/icons/{}.png", product_to_texture(&stack.resource));
    info!("Loading texture at {:?}", path);
    commands.spawn((
        stack,
        Collider::cuboid(3., 3.),
        SpriteBundle {
            texture: asset_server.load(path),
            transform: Transform::from_translation(position),
            sprite: Sprite {
                custom_size: Some(Vec2::new(6., 6.)),
                ..default()
            },
            ..default()
        },
    ));
}

pub fn drop_into_entity_inventory(
    inventories_query: &mut Query<&mut Inventory, Without<Output>>,
    collider_entity: Entity,
    stack: Stack,
    children: &Query<&Children>,
) -> bool {
    if let Ok(inventory) = inventories_query.get_mut(collider_entity).as_mut() {
        if inventory.can_add(&[(stack.resource.clone(), stack.amount)]) {
            inventory.add_stack(stack);
            return true;
        } else {
            return false;
        }
    } else {
        info!("No inventory component found on entity, searching children.");
        for child in children.iter_descendants(collider_entity) {
            if let Ok(inventory) = inventories_query.get_mut(child).as_mut() {
                if inventory.can_add(&[(stack.resource.clone(), stack.amount)]) {
                    inventory.add_stack(stack);
                    return true;
                }
            }
        }
        return false;
    }
}

pub fn take_stack_from_entity_inventory(
    inventories_query: &mut Query<&mut Inventory, (Without<Fuel>, Without<Source>)>,
    collider_entity: Entity,
    children: &Query<&Children>,
    max_size: u32,
) -> Option<Stack> {
    if let Ok(inventory) = inventories_query.get_mut(collider_entity).as_mut() {
        inventory.take_stack(max_size)
    } else {
        info!("No inventory component found on entity, searching children.");
        for child in children.iter_descendants(collider_entity) {
            if let Ok(inventory) = inventories_query.get_mut(child).as_mut() {
                if let Some(stack) = inventory.take_stack(max_size) {
                    return Some(stack);
                }
            }
        }
        None
    }
}

/// Drop a stack in a suitable inventory or drop it on the floor. Returns false when neither could
/// be done
pub fn drop_stack_at_point(
    commands: &mut Commands,
    rapier_context: &Res<RapierContext>,
    asset_server: &Res<AssetServer>,
    inventories_query: &mut Query<&mut Inventory, Without<Output>>,
    children: &Query<&Children>,
    stack: Stack,
    drop_point: Vec3,
) -> bool {
    if let Some(collider_entity) = rapier_context.intersection_with_shape(
        drop_point.xy(),
        0.,
        &Collider::ball(2.),
        QueryFilter::new(),
    ) {
        drop_into_entity_inventory(inventories_query, collider_entity, stack, children)
    } else {
        spawn_stack(commands, stack, asset_server, drop_point);
        true
    }
}
