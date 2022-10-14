use rand::seq::IteratorRandom;
use rand::Rng;
use std::collections::VecDeque;

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
};
use bevy_ecs_tilemap::prelude::*;
use futures_lite::future;
use ndarray::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PluginState {
    Finished,
}

type Chunk = Array2<Option<u32>>;

#[derive(Component)]
struct GenerateChunk(Task<Chunk>);

const GROUND: u32 = 0;
const WATER: u32 = 1;
const GRASS: u32 = 2;
const TALL_GRASS: u32 = 3;
const DEEP_WATER: u32 = 4;
const TREE: u32 = 5;
const FLOWERS: u32 = 6;
const STONE: u32 = 7;
const COAL: u32 = 8;
const IRON: u32 = 9;

async fn generate_chunk(mut chunk: Chunk) -> Chunk {
    let tilemap_size = TilemapSize {
        x: chunk.ncols() as u32 - 2,
        y: chunk.nrows() as u32 - 2,
    };

    let mut rng = rand::thread_rng();
    // Pick a random tile
    let mut tile_pos = TilePos {
        x: rng.gen_range(1..tilemap_size.x),
        y: rng.gen_range(1..tilemap_size.y),
    };
    let mut unspawned_neighbors: VecDeque<(u32, u32)> = VecDeque::new();
    let possible_neighbors = HashMap::from([
        (GROUND, HashSet::from([GROUND, WATER, GRASS, TREE])),
        (WATER, HashSet::from([GROUND, GRASS, WATER, DEEP_WATER])),
        (
            GRASS,
            HashSet::from([
                GROUND, GRASS, WATER, TALL_GRASS, TREE, FLOWERS, STONE, COAL, IRON,
            ]),
        ),
        (
            TALL_GRASS,
            HashSet::from([GRASS, TALL_GRASS, WATER, FLOWERS, STONE]),
        ),
        (DEEP_WATER, HashSet::from([WATER, DEEP_WATER])),
        (TREE, HashSet::from([GROUND, GRASS, TREE])),
        (FLOWERS, HashSet::from([GRASS, TALL_GRASS, FLOWERS])),
        (STONE, HashSet::from([GRASS, FLOWERS, TREE, STONE])),
        (COAL, HashSet::from([GRASS, COAL])),
        (IRON, HashSet::from([GRASS, IRON])),
    ]);
    let all_tiles = HashSet::from([
        WATER, GRASS, TALL_GRASS, DEEP_WATER, TREE, FLOWERS, STONE, COAL, IRON,
    ]);
    for _ in 0..tilemap_size.x * tilemap_size.y {
        debug!("Spawning new tile: {:?}", tile_pos);
        // get neighboring existing tiles in chunk
        let t_west = tile_pos.x as usize;
        let t_east = tile_pos.x as usize + 2;
        let t_north = tile_pos.y as usize;
        let t_south = tile_pos.y as usize + 2;

        if t_east >= chunk.ncols() || t_south >= chunk.nrows() {
            error!("Tile outside of chunk: {:?}", tile_pos);
            continue;
        }

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

        let tile_neighbors = &chunk.slice(s![t_west..=t_east, t_north..=t_south]);
        for ((x, y), tile) in tile_neighbors.indexed_iter() {
            let g_x = tile_pos.x as i32 + x as i32 - 1;
            let g_y = tile_pos.y as i32 + y as i32 - 1;
            if g_x < 0 || g_y < 0 || g_x >= tilemap_size.x as i32 || g_y >= tilemap_size.y as i32 {
                info!("Skipping neighbor outside of chunk: {:?}", (g_x, g_y));
                continue;
            }
            if tile.is_none() && !unspawned_neighbors.contains(&(g_x as u32, g_y as u32)) {
                unspawned_neighbors.push_back((g_x as u32, g_y as u32));
            }
        }

        if let Some(new_pos) = unspawned_neighbors.pop_front() {
            tile_pos = TilePos {
                x: new_pos.0,
                y: new_pos.1,
            };
        } else {
            error!("No unspawned neighbors found");
            break;
        }
    }

    info!("Finished generating chunk");
    chunk
}

fn setup(mut commands: Commands) {
    info!("Setting up terrain");

    let map_size = 256;
    let tilemap_size = TilemapSize {
        x: map_size,
        y: map_size,
    };

    let chunk =
        Array2::<Option<u32>>::default((tilemap_size.x as usize + 2, tilemap_size.y as usize + 2));

    info!("Starting chunk generation");
    let thread_pool = AsyncComputeTaskPool::get();
    let task = thread_pool.spawn(async move { generate_chunk(chunk).await });
    commands.spawn().insert(GenerateChunk(task));
}

fn spawn_chunk(
    mut commands: Commands,
    mut chunk_task: Query<(Entity, &mut GenerateChunk)>,
    asset_server: Res<AssetServer>,
) {
    for (entity, mut task) in &mut chunk_task {
        if let Some(chunk) = future::block_on(future::poll_once(&mut task.0)) {
            let tilemap_entity = commands.spawn().id();
            let tilemap_size = TilemapSize {
                x: chunk.ncols() as u32 - 2,
                y: chunk.nrows() as u32 - 2,
            };

            let mut tile_storage = TileStorage::empty(tilemap_size);

            for x in 0..tilemap_size.x {
                for y in 0..tilemap_size.y {
                    if let Some(texture_id) = chunk[[x as usize + 1, y as usize + 1]] {
                        let tile_pos = TilePos { x, y };
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
                    } else {
                        warn!("Tile at {:?} is empty", TilePos { x, y });
                    }
                }
            }

            let texture_handle = asset_server.load("textures/terrain.png");
            let tile_size = TilemapTileSize { x: 16., y: 16. };

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

            commands.entity(entity).remove::<GenerateChunk>();
        }
    }
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(TilemapPlugin)
            .add_state(PluginState::Finished)
            .add_startup_system(setup)
            .add_system(spawn_chunk);
    }
}
