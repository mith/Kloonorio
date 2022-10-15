use bevy::prelude::*;

use crate::types::Player;

fn keyboard_input_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut player_query: Query<&mut Transform, With<Player>>,
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

    for mut transform in player_query.iter_mut() {
        transform.translation =
            transform.translation - direction.normalize() * 200.0 * timer.delta_seconds();
    }
}

pub struct PlayerMovementPlugin;
impl Plugin for PlayerMovementPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(keyboard_input_system);
    }
}
