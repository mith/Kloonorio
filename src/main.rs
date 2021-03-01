use bevy::input::mouse::MouseMotion;
use bevy::{asset::LoadState, prelude::*, render::camera::Camera, sprite::TextureAtlasBuilder};
use bevy_tilemap::prelude::*;
use std::collections::HashMap;

fn main() {
    App::build()
        .init_resource::<SpriteHandles>()
        .init_resource::<GameState>()
        .init_resource::<CursorState>()
        .add_plugins(DefaultPlugins)
        .add_plugins(TilemapDefaultPlugins)
        .add_startup_system(setup.system())
        .add_system(mouse_world_interaction_system.system())
        .add_system(keyboard_input_system.system())
        .add_system(load.system())
        .add_system(build_world.system())
        .run();
}

#[derive(Default, Clone)]
struct SpriteHandles {
    handles: Vec<HandleUntyped>,
    atlas_loaded: bool,
}

#[derive(Default)]
struct CursorState {
    mouse_motion_event_reader: EventReader<MouseMotion>,
    under_cursor: Option<usize>,
}

#[derive(Default)]
struct GameState {
    map_loaded: bool,
    spawned: bool,
}

struct Player;

#[derive(Hash, Eq, PartialEq, Debug)]
enum Resource {
    Coal,
}

struct Inventory {
    items: HashMap<Resource, u32>
}

fn setup(
    commands: &mut Commands,
    asset_server: Res<AssetServer>,
    mut tile_sprite_handles: ResMut<SpriteHandles>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    tile_sprite_handles.handles = asset_server.load_folder("textures/terrain").unwrap();
    let character_texture_handle = asset_server.load("textures/character.png");
    asset_server.watch_for_changes().unwrap();
    commands
        .spawn(SpriteBundle {
            material: materials.add(character_texture_handle.into()),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0)),
            ..Default::default()
        })
        .with(Player)
        .with(Inventory {
            items: HashMap::new()
        })
        .with_children(|parent| {
            parent.spawn(Camera2dBundle {
                transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0)),
                ..Default::default()
            });
        });
}

fn load(
    commands: &mut Commands,
    mut sprite_handles: ResMut<SpriteHandles>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut textures: ResMut<Assets<Texture>>,
    asset_server: Res<AssetServer>,
) {
    if sprite_handles.atlas_loaded {
        return;
    }

    // Lets load all our textures from our folder!
    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    if let LoadState::Loaded =
        asset_server.get_group_load_state(sprite_handles.handles.iter().map(|handle| handle.id))
    {
        for handle in sprite_handles.handles.iter() {
            let texture = textures.get(handle).unwrap();
            texture_atlas_builder.add_texture(handle.clone_weak().typed::<Texture>(), &texture);
        }

        let texture_atlas = texture_atlas_builder.finish(&mut textures).unwrap();
        let atlas_handle = texture_atlases.add(texture_atlas);

        let tilemap = Tilemap::builder()
            .auto_chunk()
            .topology(GridTopology::Square)
            .chunk_dimensions(32, 32)
            .auto_spawn(3, 3)
            .tile_dimensions(16, 16)
            .z_layers(2)
            .add_layer(
                TilemapLayer {
                    kind: LayerKind::Dense,
                    ..Default::default()
                },
                0,
            )
            .add_layer(
                TilemapLayer {
                    kind: LayerKind::Sparse,
                    ..Default::default()
                },
                1,
            )
            .texture_atlas(atlas_handle)
            .finish()
            .unwrap();

        let tilemap_components = TilemapBundle {
            tilemap,
            transform: Default::default(),
            global_transform: Default::default(),
        };

        commands
            .spawn(tilemap_components)
            .with(Timer::from_seconds(0.075, true));

        sprite_handles.atlas_loaded = true;
        info!("Sprites loaded");
    }
}

fn build_world(
    mut game_state: ResMut<GameState>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    asset_server: Res<AssetServer>,
    mut query: Query<&mut Tilemap>,
) {
    if game_state.map_loaded {
        return;
    }

    for mut map in query.iter_mut() {
        let floor: Handle<Texture> = asset_server.get_handle("textures/terrain/ground.png");
        let coal: Handle<Texture> = asset_server.get_handle("textures/terrain/coal.png");
        let texture_atlas = texture_atlases.get(map.texture_atlas()).unwrap();
        let floor_index = texture_atlas.get_texture_index(&floor).unwrap();
        let coal_index = texture_atlas.get_texture_index(&coal).unwrap();

        let mut tiles = Vec::new();
        for y in -100..100 {
            for x in -100..100 {
                let tile = Tile {
                    point: (x, y),
                    sprite_index: floor_index,
                    z_order: 0,
                    ..Default::default()
                };
                tiles.push(tile);
            }
        }

        tiles.push(Tile {
            point: (2, 4),
            sprite_index: coal_index,
            z_order: 1,
            ..Default::default()
        });
        tiles.push(Tile {
            point: (0, 0),
            sprite_index: coal_index,
            z_order: 1,
            ..Default::default()
        });
        tiles.push(Tile {
            point: (10, 10),
            sprite_index: coal_index,
            z_order: 1,
            ..Default::default()
        });
        map.insert_tiles(tiles).unwrap();
        game_state.map_loaded = true;
        info!("Map loaded")
    }
}

fn mouse_world_interaction_system(
    mut state: Local<CursorState>,
    windows: Res<Windows>,
    mouse_button_input: Res<Input<MouseButton>>,
    camera_transforms: Query<&GlobalTransform, With<Camera>>,
    mut tilemap_query: Query<(&mut Tilemap, &GlobalTransform)>,
    mut inventory_query: Query<&mut Inventory, With<Player>>
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
            debug!("cursor_position = {}", cursor_position);
            debug!("tile_position = {:?}", tile_position);
            if let Some(tile) = tilemap.get_tile(tile_position, 1) {
                state.under_cursor = Some(tile.index);
                debug!("tile = {:?}", tile);
                if tile.index > 0 && mouse_button_input.just_pressed(MouseButton::Right) {
                    for mut inventory in inventory_query.iter_mut() {
                        let current_amount = inventory.items.entry(Resource::Coal).or_insert(0);
                        *current_amount += 1;
                        info!("Picked up 1 coal. current amount: {}", *current_amount);
                    }
                }
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

fn keyboard_input_system(
    keyboard_input: Res<Input<KeyCode>>,
    mut player_query: Query<&mut Transform, With<Player>>,
) {
    let mut direction = Vec3::new(0.0, 0.0, 0.0);
    if keyboard_input.pressed(KeyCode::A) {
        direction.x = 1.0
    }

    if keyboard_input.pressed(KeyCode::E) {
        direction.x = -1.0
    }

    if keyboard_input.pressed(KeyCode::Comma) {
        direction.y = -1.0
    }

    if keyboard_input.pressed(KeyCode::O) {
        direction.y = 1.0
    }

    if direction.length_squared() > 0.0 {
        for mut transform in player_query.iter_mut() {
            transform.translation = transform.translation - direction.normalize() * 8.0;
        }
    }
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
