use bevy::prelude::*;

use bevy_rapier2d::control::KinematicCharacterController;
use kloonorio_core::{player::Player, types::AppState};
use kloonorio_terrain::CursorWorldPos;

use crate::{
    biter::Biter,
    shoot::{Gun, Target},
};

pub struct PlayerControlPlugin;

impl Plugin for PlayerControlPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (keyboard_movement_system, keyboard_shoot_system).run_if(in_state(AppState::Running)),
        );
    }
}

fn keyboard_movement_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut player_query: Query<&mut KinematicCharacterController, With<Player>>,
    timer: Res<Time>,
) {
    let mut direction = Vec3::new(0.0, 0.0, 0.0);
    if keyboard_input.pressed(KeyCode::Comma) {
        direction.y = -1.0
    }

    if keyboard_input.pressed(KeyCode::A) {
        direction.x = 1.0
    }

    if keyboard_input.pressed(KeyCode::O) {
        direction.y = 1.0
    }

    if keyboard_input.pressed(KeyCode::E) {
        direction.x = -1.0
    }

    if direction.length_squared() == 0.0 {
        return;
    }

    let velocity = direction.normalize() * 10.0 * timer.delta_seconds();

    for mut controller in player_query.iter_mut() {
        controller.translation = Some(-velocity.xy());
    }
}

fn keyboard_shoot_system(
    keyboard_input: Res<Input<KeyCode>>,
    player_query: Query<Entity, (With<Player>, With<Gun>)>,
    biter_query: Query<(Entity, &GlobalTransform), With<Biter>>,
    cursor_pos: Res<CursorWorldPos>,
    mut commands: Commands,
) {
    if keyboard_input.pressed(KeyCode::Space) {
        // Find the biter closest to the cursor
        let closest_biter = biter_query
            .iter()
            .map(|(entity, transform)| {
                (
                    entity,
                    transform.translation().xy().distance(cursor_pos.0.xy()),
                )
            })
            .min_by(|(_, distance_a), (_, distance_b)| distance_a.partial_cmp(distance_b).unwrap())
            .map(|(entity, _)| entity);

        if let Some(biter) = closest_biter {
            for player in player_query.iter() {
                commands.entity(player).insert(Target(biter));
            }
        }
    } else {
        for player in player_query.iter() {
            commands.entity(player).remove::<Target>();
        }
    }
}
