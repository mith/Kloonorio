use rand::seq::IteratorRandom;
use rand::Rng;
use std::collections::VecDeque;

use bevy::{
    asset::LoadState,
    prelude::*,
    sprite::TextureAtlasBuilder,
    utils::{HashMap, HashSet},
};
use bevy_ecs_tilemap::prelude::*;
use ndarray::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PluginState {
    Finished,
}

const GROUND: u32 = 0;
const WATER: u32 = 1;
const GRASS: u32 = 2;
const TALL_GRASS: u32 = 3;
const DEEP_WATER: u32 = 4;
const TREE: u32 = 5;
const FLOWERS: u32 = 6;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    info!("Setting up terrain");

    let texture_handle = asset_server.load("textures/terrain.png");

    let tile_size = TilemapTileSize { x: 16., y: 16. };

    info!("Creating tilemap");
    let tilemap_entity = commands.spawn().id();
    let map_size = 8;
    let tilemap_size = TilemapSize {
        x: map_size,
        y: map_size,
    };
    let mut tile_storage = TileStorage::empty(tilemap_size);

    let mut rng = rand::thread_rng();
    // Pick a random tile
    let mut tile_pos = TilePos {
        x: rng.gen_range(1..tilemap_size.x),
        y: rng.gen_range(0..tilemap_size.y),
    };
    let mut unspawned_neighbors: VecDeque<(u32, u32)> = VecDeque::new();
    let possible_neighbors = HashMap::from([
        (GROUND, HashSet::from([GROUND, WATER, GRASS, TREE])),
        (WATER, HashSet::from([GROUND, GRASS, WATER, DEEP_WATER])),
        (
            GRASS,
            HashSet::from([GROUND, GRASS, WATER, TALL_GRASS, TREE, FLOWERS]),
        ),
        (
            TALL_GRASS,
            HashSet::from([GRASS, TALL_GRASS, WATER, FLOWERS]),
        ),
        (DEEP_WATER, HashSet::from([WATER, DEEP_WATER])),
        (TREE, HashSet::from([GROUND, GRASS, TREE])),
        (FLOWERS, HashSet::from([GRASS, TALL_GRASS, FLOWERS])),
    ]);
    let mut chunk =
        Array2::<Option<u32>>::default((tilemap_size.x as usize + 2, tilemap_size.y as usize + 2));
    let all_tiles = HashSet::from([WATER, GRASS, TALL_GRASS, DEEP_WATER, TREE, FLOWERS]);
    for _ in 0..tilemap_size.x * tilemap_size.y {
        debug!("Spawning new tile: {:?}", tile_pos);
        // get neighboring existing tiles in chunk
        let t_west = tile_pos.x as usize;
        let t_east = tile_pos.x as usize + 2;
        let t_north = tile_pos.y as usize;
        let t_south = tile_pos.y as usize + 2;

        let neighbors = &chunk.slice(s![t_west..=t_east, t_north..=t_south]);
        debug!("Neighbors: {:?}", neighbors);

        let possible: HashSet<u32> = neighbors
            .iter()
            .flatten()
            .map(|tile| {
                possible_neighbors
                    .get(&tile)
                    .expect("Tile type has no possible neighbors set")
            })
            .fold(all_tiles.clone(), |acc, poss_tiles| {
                acc.intersection(&poss_tiles).map(|n| *n).collect()
            });

        debug!("Possible tiles: {:?}", possible);
        if possible.len() == 0 {
            error!("No possible tiles found")
        }

        let texture_id = *possible.iter().choose(&mut rng).unwrap();
        debug!("Chosen tile: {:?}", texture_id);

        chunk[[tile_pos.x as usize + 1, tile_pos.y as usize + 1]] = Some(texture_id);

        let tile_entity = commands
            .spawn()
            .insert_bundle(TileBundle {
                position: tile_pos,
                tilemap_id: TilemapId(tilemap_entity),
                texture: TileTexture(texture_id),
                ..default()
            })
            .id();

        tile_storage.set(&tile_pos, Some(tile_entity));

        let tile_neighbors = tile_storage.get_neighboring_pos(&tile_pos);
        debug!("Tile neighbors: {:?}", tile_neighbors);
        for neighbor_pos in tile_neighbors {
            if let Some(pos) = neighbor_pos {
                if tile_storage.get(&pos).is_none()
                    && !unspawned_neighbors.contains(&(pos.x, pos.y))
                {
                    debug!("Adding neighbor: {:?}", pos);
                    unspawned_neighbors.push_back((pos.x, pos.y));
                }
            }
        }

        if let Some(new_pos) = unspawned_neighbors.pop_front() {
            tile_pos = TilePos {
                x: new_pos.0,
                y: new_pos.1,
            };
        } else {
            warn!("No unspawned neighbors found");
            break;
        }
    }

    if !unspawned_neighbors.is_empty() {
        warn!("Not all tiles spawned");
    }

    info!("Adding tilemap to world");
    commands
        .entity(tilemap_entity)
        .insert_bundle(TilemapBundle {
            grid_size: TilemapGridSize {
                x: tilemap_size.x as f32,
                y: tilemap_size.y as f32,
            },
            size: tilemap_size,
            storage: tile_storage,
            texture: TilemapTexture(texture_handle),
            tile_size,
            transform: bevy_ecs_tilemap::helpers::get_centered_transform_2d(
                &tilemap_size,
                &tile_size,
                0.0,
            ),
            ..default()
        });
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(TilemapPlugin)
            .add_state(PluginState::Finished)
            .add_system_set(SystemSet::on_enter(PluginState::Finished).with_system(setup));
    }
}
