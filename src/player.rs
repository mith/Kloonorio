use bevy::{
    app::{App, Plugin},
    asset::AssetServer,
    core::Name,
    core_pipeline::core_2d::Camera2dBundle,
    ecs::{
        component::Component,
        schedule::OnEnter,
        system::{Commands, Res},
    },
    hierarchy::BuildChildren,
    math::Vec2,
    prelude::default,
    render::camera::OrthographicProjection,
    sprite::{Sprite, SpriteBundle},
    transform::components::Transform,
};

use crate::{
    inventory::Inventory,
    types::{AppState, CraftingQueue, Product},
    ui::{hotbar::Hotbar, inventory_grid::Hand},
    ysort::YSort,
};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_player);
    }
}

#[derive(Component)]
pub struct Player;
fn spawn_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut inventory = Inventory::new(100);
    inventory.add_item(Product::Structure("Wooden chest".into()), 100);
    inventory.add_item(Product::Structure("Burner mining drill".into()), 100);
    inventory.add_item(Product::Structure("Stone furnace".into()), 100);
    inventory.add_item(Product::Structure("Burner inserter".into()), 100);
    inventory.add_item(Product::Intermediate("Coal".into()), 200);
    inventory.add_item(Product::Intermediate("Iron ore".into()), 200);
    inventory.add_item(Product::Structure("Transport belt".into()), 200);
    inventory.add_item(Product::Structure("Burner assembling machine".into()), 100);
    commands
        .spawn((
            Name::new("Player"),
            YSort { base_layer: 1.0 },
            SpriteBundle {
                texture: asset_server.load("textures/character.png"),
                transform: Transform::from_xyz(0.0, 0.0, 1.0),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(2., 2.)),
                    ..default()
                },
                ..default()
            },
            Player,
            Hand::default(),
            inventory,
            CraftingQueue::default(),
            Hotbar::new(5),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Player camera"),
                Camera2dBundle {
                    transform: Transform::from_xyz(0.0, 0.0, 500.0),
                    projection: OrthographicProjection {
                        scale: 0.01,
                        ..Default::default()
                    },
                    ..default()
                },
            ));
        });
}
