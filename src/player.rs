use bevy::{
    app::{App, Plugin},
    asset::AssetServer,
    core::Name,
    core_pipeline::core_2d::Camera2dBundle,
    ecs::{
        schedule::OnEnter,
        system::{Commands, Res},
    },
    hierarchy::BuildChildren,
    math::Vec2,
    prelude::default,
    render::camera::OrthographicProjection,
    sprite::{Sprite, SpriteBundle},
    transform::{components::Transform, TransformBundle},
};
use bevy_rapier2d::{control::KinematicCharacterController, geometry::Collider};
use kloonorio_core::{health::Health, item::Item, player::Player};

use crate::{shoot::Gun, ysort::YSort};
use kloonorio_core::{
    inventory::Inventory,
    types::{AppState, CraftingQueue},
};
use kloonorio_ui::{hotbar::Hotbar, inventory_grid::Hand};

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_player);
    }
}

fn spawn_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut inventory = Inventory::new(100);
    inventory.add_item(&Item::new("Wooden chest"), 100);
    inventory.add_item(&Item::new("Burner mining drill"), 100);
    inventory.add_item(&Item::new("Stone furnace"), 100);
    inventory.add_item(&Item::new("Burner inserter"), 100);
    inventory.add_item(&Item::new("Coal"), 200);
    inventory.add_item(&Item::new("Iron plate"), 200);
    inventory.add_item(&Item::new("Transport belt"), 200);
    inventory.add_item(&Item::new("Burner assembling machine"), 100);
    commands
        .spawn((
            Name::new("Player"),
            YSort { base_layer: 1.0 },
            TransformBundle::from_transform(Transform::from_xyz(0.0, 0.0, 1.0)),
            Player,
            Health::new(100),
            Hand::default(),
            Gun {
                range: 10.,
                damage: 10,
                cooldown: 1.,
            },
            inventory,
            CraftingQueue::default(),
            KinematicCharacterController { ..default() },
            Collider::ball(0.3),
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
            parent.spawn((
                Name::new("Player sprite"),
                SpriteBundle {
                    texture: asset_server.load("textures/character.png"),
                    transform: Transform::from_xyz(0.0, 0.4, 0.0),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(2., 2.)),
                        ..default()
                    },
                    ..default()
                },
            ));
        });
}
