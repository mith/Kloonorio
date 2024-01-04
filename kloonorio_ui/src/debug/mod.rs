use bevy::prelude::*;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::render::{DebugRenderContext, RapierDebugRenderPlugin};

use kloonorio_terrain::TerrainDebug;

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
            (toggle_physics_debug, toggle_inspector, toggle_terrain_debug).in_set(DebugSet),
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
    if keyboard_input.just_pressed(KeyCode::F2) {
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
    if keyboard_input.just_pressed(KeyCode::F3) {
        if debug_render.enabled {
            info!("Disabling physics debug");
            debug_render.enabled = false;
        } else {
            info!("Enabling physics debug");
            debug_render.enabled = true;
        }
    }
}

fn toggle_terrain_debug(
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    terrain_debug: Option<Res<TerrainDebug>>,
) {
    if keyboard_input.just_pressed(KeyCode::F4) {
        if terrain_debug.is_some() {
            info!("Disabling terrain debug");
            commands.remove_resource::<TerrainDebug>();
        } else {
            info!("Enabling terrain debug");
            commands.init_resource::<TerrainDebug>();
        }
    }
}
