use bevy::prelude::*;

use bevy_rapier2d::control::KinematicCharacterController;
use kloonorio_core::{player::Player, types::AppState};

fn keyboard_input_system(
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

pub struct PlayerMovementPlugin;
impl Plugin for PlayerMovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            keyboard_input_system.run_if(in_state(AppState::Running)),
        );
    }
}
