use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::render::{DebugRenderContext, RapierDebugRenderPlugin};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(WorldInspectorPlugin::default().run_if(resource_exists::<Inspector>()));

        app.add_plugins(RapierDebugRenderPlugin {
            enabled: false,
            ..default()
        })
        .add_systems(
            Update,
            (toggle_physics_debug, toggle_inspector).in_set(DebugSet),
        );
    }
}

#[derive(SystemSet, Hash, PartialEq, Eq, Clone, Debug)]
pub struct DebugSet;

#[derive(Resource)]
struct Inspector;

fn toggle_inspector(
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    maybe_inspector: Option<Res<Inspector>>,
) {
    if keyboard_input.just_pressed(KeyCode::F3) {
        if maybe_inspector.is_some() {
            info!("Disabling inspector");
            commands.remove_resource::<Inspector>();
        } else {
            info!("Enabling inspector");
            commands.insert_resource(Inspector);
        }
    }
}

fn toggle_physics_debug(
    mut debug_render: ResMut<DebugRenderContext>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    if keyboard_input.just_pressed(KeyCode::F2) {
        info!("Toggling physics debug");
        debug_render.enabled = !debug_render.enabled;
    }
}
