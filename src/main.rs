use bevy::{prelude::*, render::camera::Camera};
use bevy_egui::{egui, EguiContext, EguiPlugin};
use bevy_tilemap::prelude::*;

mod inventory;
mod player_movement;
mod terrain;
mod types;

use inventory::{Inventory, InventoryPlugin};
use player_movement::PlayerMovementPlugin;
use terrain::TerrainPlugin;
use types::{CursorState, GameState, Player, SpriteHandles};

fn main() {
    let mut app = App::build();
        app.init_resource::<SpriteHandles>()
        .init_resource::<GameState>()
        .init_resource::<CursorState>()
        .add_plugins(DefaultPlugins)
        .add_plugins(TilemapDefaultPlugins)
        .add_plugin(EguiPlugin)
        .add_plugin(InventoryPlugin)
        .add_plugin(TerrainPlugin)
        .add_plugin(PlayerMovementPlugin)
        .add_startup_system(setup.system())
        .add_system(mouse_world_interaction_system.system())
        .add_system(debug_ui.system())
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut tile_sprite_handles: ResMut<SpriteHandles>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    tile_sprite_handles.handles = asset_server.load_folder("textures/terrain").unwrap();
    let character_texture_handle = asset_server.load("textures/character.png");
    asset_server.watch_for_changes().unwrap();
    commands
        .spawn_bundle(OrthographicCameraBundle::new_2d())
        .insert(Player)
        .insert(Inventory::new(12))
        .with_children(|parent| {
            parent.spawn_bundle(SpriteBundle {
                material: materials.add(character_texture_handle.into()),
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
    mut tilemap_query: Query<(&mut Tilemap, &GlobalTransform)>,
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
        for (mut tilemap, tilemap_transform) in tilemap_query.iter_mut() {
            let tile_position = get_tile_position_under_cursor(
                cursor_position,
                camera_transform,
                tilemap_transform,
                tilemap.tile_width(),
            );
            trace!("cursor_position = {}", cursor_position);
            trace!("tile_position = {:?}", tile_position);
            if let Some(tile) = tilemap.get_tile(tile_position, 1) {
                state.under_cursor = Some(tile.index);
                debug!("tile = {:?}", tile);
                // if tile.index > 0 && mouse_button_input.just_pressed(MouseButton::Right) {
                //     for mut inventory in inventory_query.iter_mut() {
                //         let current_amount = inventory.items.entry(Resource::Coal).or_insert(0);
                //         *current_amount += 1;
                //         info!("Picked up 1 coal. current amount: {}", *current_amount);
                //     }
                // }
            }
        }
    }
}

fn get_tile_position_under_cursor(
    cursor_position: Vec3,
    camera_transform: &GlobalTransform,
    tilemap_transform: &GlobalTransform,
    tile_size: u32,
) -> (i32, i32) {
    let translation = (camera_transform.mul_vec3(cursor_position)) - tilemap_transform.translation;
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
        let tile_position = get_tile_position_under_cursor(
            cursor_position,
            &camera_transform,
            &tilemap_transform,
            tile_size,
        );
        assert_eq!((0, 0), tile_position);
    }
}
