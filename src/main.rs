use bevy::{
    asset::AssetServerSettings,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseWheel,
    prelude::*,
    render::texture::ImageSettings,
};
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_inspector_egui::WorldInspectorPlugin;

mod inventory;
mod player_movement;
mod terrain;
mod types;

use inventory::{Inventory, InventoryPlugin};
use player_movement::PlayerMovementPlugin;
use terrain::TerrainPlugin;
use types::{AppState, CursorState, GameState, Player};

fn main() {
    let mut app = App::new();
    app.insert_resource(AssetServerSettings {
        watch_for_changes: true,
        ..default()
    })
    .insert_resource(ImageSettings::default_nearest())
    .init_resource::<GameState>()
    .init_resource::<CursorState>()
    // .add_plugin(LogDiagnosticsPlugin::default())
    .add_plugin(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(DefaultPlugins)
    // .add_plugin(WorldInspectorPlugin::new())
    .add_plugin(EguiPlugin)
    .add_plugin(InventoryPlugin)
    .add_plugin(TerrainPlugin)
    .add_plugin(PlayerMovementPlugin)
    .add_state(AppState::Setup)
    .add_startup_system(setup)
    .add_system(camera_zoom)
    .add_system(performance_ui)
    .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    asset_server.watch_for_changes().unwrap();
    commands
        .spawn_bundle(SpriteBundle {
            texture: asset_server.load("textures/character.png"),
            transform: Transform::from_xyz(0.0, 0.0, 1.0),
            ..Default::default()
        })
        .insert(Player)
        .insert(Inventory::new(12))
        .with_children(|parent| {
            parent.spawn_bundle(Camera2dBundle {
                transform: Transform::from_xyz(0.0, 0.0, 500.0),
                projection: OrthographicProjection {
                    scale: 0.3,
                    ..Default::default()
                },
                ..default()
            });
        });
}

fn camera_zoom(
    mut query: Query<&mut OrthographicProjection>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
) {
    for mut projection in &mut query {
        for event in mouse_wheel_events.iter() {
            projection.scale -= event.y * 0.1;
            projection.scale = projection.scale.max(0.1).min(0.4);
        }
    }
}

fn performance_ui(mut egui_context: ResMut<EguiContext>, diagnostics: Res<Diagnostics>) {
    egui::Window::new("Performance").show(egui_context.ctx_mut(), |ui| {
        if let Some(diagnostic) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = diagnostic.average() {
                ui.label(format!("FPS: {:.2}", average));
            }
        }
    });
}
