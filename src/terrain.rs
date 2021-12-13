use bevy::{asset::LoadState, prelude::*, sprite::TextureAtlasBuilder};
use bevy_tilemap::{
    event::TilemapChunkEvent, point::Point2, prelude::GridTopology, prelude::LayerKind,
    prelude::TilemapBundle, Tile, Tilemap, TilemapLayer,
};

use crate::types::{GameState, SpriteHandles};

fn load(
    mut commands: Commands,
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
            .topology(GridTopology::Square)
            .chunk_dimensions(32, 32, 1)
            .texture_dimensions(16, 16)
            .auto_chunk()
            .auto_spawn(2, 2)
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
            visible: Visible {
                is_visible: true,
                is_transparent: true,
            },
            transform: Default::default(),
            global_transform: Default::default(),
        };

        commands
            .spawn_bundle(tilemap_components);
            // .with(Timer::from_seconds(0.075, true));

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

        tiles.push(Tile {
            point: (2, 4),
            sprite_index: coal_index,
            sprite_order: 1,
            ..Default::default()
        });
        tiles.push(Tile {
            point: (0, 0),
            sprite_index: coal_index,
            sprite_order: 1,
            ..Default::default()
        });
        tiles.push(Tile {
            point: (10, 10),
            sprite_index: coal_index,
            sprite_order: 1,
            ..Default::default()
        });
        map.insert_tiles(tiles).unwrap();
        game_state.map_loaded = true;
        info!("Map loaded")
    }
}

fn chunk_generation_system(
    mut tilemap_query: Query<(&mut Tilemap, &GlobalTransform)>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    asset_server: Res<AssetServer>,
) {
    for (mut tilemap, _) in tilemap_query.iter_mut() {
        let floor: Handle<Texture> = asset_server.get_handle("textures/terrain/ground.png");

        let texture_atlas = texture_atlases.get(tilemap.texture_atlas()).unwrap();
        let mut reader = tilemap.chunk_events().get_reader();
        let floor_index = texture_atlas.get_texture_index(&floor).unwrap();

        let mut tiles = Vec::new();

        for event in reader.iter(&tilemap.chunk_events()) {
            debug!("chunk event: {:?}", event);
            if let TilemapChunkEvent::Spawned { point } = event {
                tiles.append(&mut populate_chunk(*point, &tilemap, floor_index));
            }
        }

        tilemap.insert_tiles(tiles).unwrap();
    }
}

fn populate_chunk(point: Point2, tilemap: &Tilemap, floor_index: usize) -> Vec<Tile<(i32, i32)>> {
    let mut tiles = Vec::new();
    let chunk_x = point.x * tilemap.chunk_width() as i32;
    let chunk_y = point.y * tilemap.chunk_height() as i32;
    for x in 0..tilemap.chunk_width() as i32 {
        for y in 0..tilemap.chunk_height() as i32 {
            let tile = Tile {
                point: (chunk_x + x, chunk_y + y),
                sprite_index: floor_index,
                sprite_order: 0,
                ..Default::default()
            };
            tiles.push(tile);
        }
    }
    return tiles;
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut AppBuilder) {
        app.add_system(load.system())
            .add_system(build_world.system())
            .add_system(chunk_generation_system.system());
    }
}
