use bevy::{prelude::*, render::camera::Camera, diagnostic::{LogDiagnosticsPlugin, FrameTimeDiagnosticsPlugin}};
use bevy_ecs_tilemap::prelude::*;
use bevy_egui::{egui, EguiContext, EguiPlugin};

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
    app.init_resource::<GameState>()
        .init_resource::<CursorState>()
        .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(DefaultPlugins)
        .add_plugin(TilemapPlugin)
        .add_plugin(EguiPlugin)
        .add_plugin(InventoryPlugin)
        .add_plugin(TerrainPlugin)
        .add_plugin(PlayerMovementPlugin)
        .add_state(AppState::Setup)
        .add_startup_system(setup)
        .add_system(mouse_world_interaction_system)
        .add_system(debug_ui)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let character_texture_handle = asset_server.load("textures/character.png");
    asset_server.watch_for_changes().unwrap();
    commands
        .spawn_bundle(OrthographicCameraBundle::new_2d())
        .insert(Player)
        .insert(Inventory::new(12))
        .with_children(|parent| {
            parent.spawn_bundle(SpriteBundle {
                texture: character_texture_handle,
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
                ..Default::default()
            });
        });
}

fn mouse_world_interaction_system(
    mut state: Local<CursorState>,
    windows: Res<Windows>,
    mouse_button_input: Res<Input<MouseButton>>,
    camera_transforms: Query<&GlobalTransform, With<Camera>>,
    mut tile_query: Query<&mut Tile>,
    mut map_query: MapQuery,
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

    for camera_transform in camera_transforms.iter() {
        let tile_position = get_tile_position_under_cursor(cursor_position, camera_transform, 16);
        info!("cursor_position = {}", cursor_position);
        info!("tile_position = {:?}", tile_position);
        if let Ok(Ok(tile)) = map_query
            .get_tile_entity(
                TilePos(tile_position.0 as u32, tile_position.1 as u32),
                0u16,
                0u16,
            )
            .map(|t| tile_query.get(t))
        {
            state.under_cursor = Some(tile.texture_index.into());
            info!("tile = {:?}", tile);
        }
        // if tile.index > 0 && mouse_button_input.just_pressed(MouseButton::Right) {
        //     for mut inventory in inventory_query.iter_mut() {
        //         let current_amount = inventory.items.entry(Resource::Coal).or_insert(0);
        //         *current_amount += 1;
        //         info!("Picked up 1 coal. current amount: {}", *current_amount);
        //     }
        // }
    }
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
    egui::Window::new("Debug").show(egui_context.ctx(), |ui| {});
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
