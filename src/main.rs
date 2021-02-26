use bevy::input::mouse::MouseMotion;
use bevy::{asset::LoadState, prelude::*, sprite::TextureAtlasBuilder};
use bevy_tilemap::prelude::*;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() {
        App::build()
        .add_resource(Msaa { samples: 4 })
        // .init_resource::<SpriteHandles>()
        // .init_resource::<GameState>()
        .add_plugins(DefaultPlugins)
        // .add_plugins(TilemapDefaultPlugins)
        .add_startup_system(setup.system())
        .add_system(camera_movement_system.system())
        .add_system(keyboard_input_system.system())
        // .add_system(load.system())
        // .add_system(build_world.system())
        .run();
}

#[derive(Default, Clone)]
struct SpriteHandles {
    handles: Vec<HandleUntyped>,
    atlas_loaded: bool,
}

#[derive(Default)]
struct GameState {
    mouse_motion_event_reader: EventReader<MouseMotion>,
    map_loaded: bool,
    spawned: bool,
}

struct Player;

fn setup(
    commands: &mut Commands,
    asset_server: Res<AssetServer>,
    // mut tile_sprite_handles: ResMut<SpriteHandles>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // tile_sprite_handles.handles = asset_server.load_folder("textures/terrain").unwrap();
    let character_texture_handle = asset_server.load("textures/character.png");
    // asset_server.watch_for_changes().unwrap();
    commands
        .spawn(SpriteBundle {
            material: materials.add(character_texture_handle.into()),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
            ..Default::default()
        })
        .with(Player)
        .with_children(|parent| {
            parent.spawn(Camera2dBundle {
                // transform: Transform::from_translation(Vec3::new(0.0, 0.0, 3.0)),
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
            .tile_dimensions(16, 16)
            .z_layers(2)
            .add_layer(LayerKind::Dense, 0)
            .add_layer(LayerKind::Sparse, 1)
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

        map.insert_chunk((0, 0)).unwrap();

        let mut tiles = Vec::new();
        for y in -100..100 {
            for x in -100..100 {
                let tile = Tile::new((x, y), floor_index);
                tiles.push(tile);
            }
        }

        tiles.push(Tile::with_z_order((2, 4), coal_index, 1));
        tiles.push(Tile::with_z_order((0, 0), coal_index, 1));
        tiles.push(Tile::with_z_order((10, 10), coal_index, 1));
        map.insert_tiles(tiles).unwrap();

//         let load_chunks = 5;

//         for x in -load_chunks..load_chunks {
//             for y in -load_chunks..load_chunks {
//                 map.spawn_chunk((x, y)).unwrap();
//             }
//         }

        game_state.map_loaded = true;
        info!("Map loaded")
    }
}

fn camera_movement_system(mut state: Local<GameState>, mouse_motion_events: Res<Events<MouseMotion>>) {
    for event in state.mouse_motion_event_reader.iter(&mouse_motion_events) {
        trace!("{:?}", event);
    }
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

    for mut transform in player_query.iter_mut() {
        transform.translation = transform.translation - direction.normalize() * 8.0;
    }
}
