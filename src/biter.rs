use bevy::{
    app::{App, FixedUpdate, Plugin},
    core::Name,
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        schedule::{common_conditions::in_state, IntoSystemConfigs, OnEnter},
        system::{Commands, Query},
    },
    hierarchy::BuildChildren,
    math::{Vec2, Vec3Swizzles},
    prelude::default,
    render::color::Color,
    sprite::{Sprite, SpriteBundle},
    transform::{
        components::{GlobalTransform, Transform},
        TransformBundle,
    },
};
use bevy_rapier2d::{control::KinematicCharacterController, geometry::Collider};
use kloonorio_core::{player::Player, types::AppState};

use crate::{
    health::Health,
    shoot::{Gun, Target},
    ysort::YSort,
};

pub struct BiterPlugin;

impl Plugin for BiterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::Running), spawn_biter)
            .add_systems(
                FixedUpdate,
                (move_to_player, attack_player).run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Component)]
pub struct Biter;

fn spawn_biter(mut commands: Commands) {
    commands
        .spawn((
            Name::new("Biter"),
            YSort { base_layer: 1.0 },
            TransformBundle::from_transform(Transform::from_xyz(10., 0., 1.)),
            Biter,
            Health::new(100),
            Gun {
                range: 1.,
                damage: 20,
                cooldown: 1.,
            },
            Collider::ball(0.4),
            KinematicCharacterController::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Biter sprite"),
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::RED,
                        custom_size: Some(Vec2::new(1., 1.)),
                        ..default()
                    },
                    ..default()
                },
            ));
        });
}

fn move_to_player(
    player_query: Query<&GlobalTransform, With<Player>>,
    mut biter_query: Query<(&mut KinematicCharacterController, &GlobalTransform), With<Biter>>,
) {
    let player_position = player_query.single().translation().xy();

    for (mut biter_controller, biter_transform) in biter_query.iter_mut() {
        let direction_to_player = player_position - biter_transform.translation().xy();

        if direction_to_player.length() > 0.1 {
            biter_controller.translation = Some(direction_to_player.normalize() * 0.1);
        }
    }
}

fn attack_player(
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    biter_query: Query<Entity, (With<Biter>, Without<Target>)>,
) {
    for biter in biter_query.iter() {
        let player = player_query.iter().next().unwrap();
        commands.entity(biter).insert(Target(player));
    }
}
