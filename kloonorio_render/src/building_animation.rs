use bevy::{
    app::{App, FixedUpdate},
    core::Name,
    ecs::{
        query::{Or, With, Without},
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{ParamSet, Query, Res},
    },
};
use kloonorio_core::{
    structure::Structures,
    types::{AppState, Powered, Working},
};

use crate::isometric_sprite::IsometricSprite;

pub struct BuildingAnimationPlugin;

impl bevy::app::Plugin for BuildingAnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            FixedUpdate,
            working_texture.run_if(in_state(AppState::Running)),
        );
    }
}

fn working_texture(
    mut buildings: ParamSet<(
        Query<(&mut IsometricSprite, &Name), (With<Powered>, With<Working>)>,
        Query<(&mut IsometricSprite, &Name), Or<(Without<Powered>, Without<Working>)>>,
    )>,
    structures: Res<Structures>,
) {
    for (mut active_sprite, name) in buildings.p0().iter_mut() {
        let structure = structures.get(&name.to_string()).unwrap();
        if structure.animated {
            let current_index = active_sprite.custom_texture_index.unwrap_or(0);
            if current_index < 90 {
                active_sprite.custom_texture_index = Some(current_index + 1);
            } else {
                // Reset to frame 30 for a looping animation
                active_sprite.custom_texture_index = Some(30);
            }
        } else {
            active_sprite.custom_texture_index = None;
        }
    }

    for (mut unpowered_sprite, name) in buildings.p1().iter_mut() {
        let Some(structure) = structures.get(&name.to_string()) else {
            unpowered_sprite.custom_texture_index = None;
            continue;
        };
        if structure.animated {
            let current_index = unpowered_sprite.custom_texture_index.unwrap_or(0);
            if (1..90).contains(&current_index) {
                unpowered_sprite.custom_texture_index = Some(90);
            } else if (90..119).contains(&current_index) {
                unpowered_sprite.custom_texture_index = Some(current_index + 1);
            } else {
                unpowered_sprite.custom_texture_index = None;
            }
        } else {
            unpowered_sprite.custom_texture_index = None;
        }
    }
}
