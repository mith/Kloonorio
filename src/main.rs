use bevy::{
    asset::AssetServerSettings,
    diagnostic::LogDiagnosticsPlugin,
    input::mouse::MouseWheel,
    prelude::*,
    render::{camera::Camera, texture::ImageSettings},
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
    .add_plugin(LogDiagnosticsPlugin::default())
    // .add_plugin(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(DefaultPlugins)
    // .add_plugin(WorldInspectorPlugin::new())
    .add_plugin(InventoryPlugin)
    .add_plugin(TerrainPlugin)
    .add_plugin(PlayerMovementPlugin)
    .add_state(AppState::Setup)
    .add_startup_system(setup)
    .add_system(mouse_world_interaction_system) // .add_system(debug_ui)
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
    mut query: Query<(&mut Transform, &mut OrthographicProjection)>,
    mut state: ResMut<State<AppState>>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
) {
    for (mut transform, mut projection) in query.iter_mut() {
        for event in mouse_wheel_events.iter() {
            projection.scale += event.y * 0.1;
            transform.translation.z += event.y * 0.1;
        }
    }
}

fn mouse_world_interaction_system(
    mut state: Local<CursorState>,
    windows: Res<Windows>,
    mouse_button_input: Res<Input<MouseButton>>,
    camera_transforms: Query<&GlobalTransform, With<Camera>>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
) {
    let maybe_window: Option<Vec3> = windows.get_primary().and_then(|window| {
        window.cursor_position().map(|cursor_position| {
            Vec3::new(
                cursor_position.x - window.width() / 2.0,
                cursor_position.y - window.height() / 2.0,
                0.0,
            )
        })
    });
    let cursor_position = if let Some(window) = maybe_window {
        window
    } else {
        return;
    };

    // for camera_transform in camera_transforms.iter() {
    //     let tile_position = get_tile_position_under_cursor(cursor_position, camera_transform, 16);
    //     info!("cursor_position = {}", cursor_position);
    //     info!("tile_position = {:?}", tile_position);
    //     if let Ok(Ok(tile)) = map_query
    //         .get_tile_entity(
    //             TilePos(tile_position.0 as u32, tile_position.1 as u32),
    //             0u16,
    //             0u16,
    //         )
    //         .map(|t| tile_query.get(t))
    //     {
    //         state.under_cursor = Some(tile.texture_index.into());
    //         info!("tile = {:?}", tile);
    //     }
    //     // if tile.index > 0 && mouse_button_input.just_pressed(MouseButton::Right) {
    //     //     for mut inventory in inventory_query.iter_mut() {
    //     //         let current_amount = inventory.items.entry(Resource::Coal).or_insert(0);
    //     //         *current_amount += 1;
    //     //         info!("Picked up 1 coal. current amount: {}", *current_amount);
    //     //     }
    //     // }
    // }
}

fn get_tile_position_under_cursor(
    cursor_position: Vec3,
    camera_transform: &GlobalTransform,
    tile_size: u32,
) -> (i32, i32) {
    let translation = (camera_transform.mul_vec3(cursor_position));
    let point_x = translation.x / tile_size as f32;
    let point_y = translation.y / tile_size as f32;
    return (point_x.floor() as i32, point_y.floor() as i32);
}

fn debug_ui(
    mut egui_context: ResMut<EguiContext>,
    mut player_query: Query<&mut Transform, With<Player>>,
    inventory_query: Query<&Inventory, With<Player>>,
) {
    // egui::Window::new("Debug").show(egui_context.ctx(), |ui| {});
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_tile_position_under_cursor() {
        let cursor_position = Vec3::new(3.0, 4.0, 0.0);
        let camera_transform = GlobalTransform::from_translation(-cursor_position);
        let tilemap_transform = GlobalTransform::identity();
        let tile_size = 16;
        let tile_position =
            get_tile_position_under_cursor(cursor_position, &camera_transform, &tilemap_transform);
        assert_eq!((0, 0), tile_position);
    }
}
