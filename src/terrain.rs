use bevy_egui::EguiContext;
use rand::seq::IteratorRandom;
use rand::Rng;
use std::collections::VecDeque;

use bevy::{
    math::Vec3Swizzles,
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
};
use bevy_ecs_tilemap::prelude::*;
use futures_lite::future;
use ndarray::prelude::*;

use crate::types::Player;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(TilemapPlugin)
            .insert_resource(ChunkManager::default())
            .insert_resource(TerrainSettings {
                chunk_spawn_radius: 5,
            })
            .add_system(spawn_chunks_around_camera)
            .add_system(spawn_chunk.after(spawn_chunks_around_camera))
            .add_system(despawn_outofrange_chunks)
            .add_system(debug_ui);
    }
}

const CHUNK_SIZE: UVec2 = UVec2 { x: 8, y: 8 };
const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16., y: 16. };

struct TerrainSettings {
    chunk_spawn_radius: i32,
}

type Chunk = Array2<Option<u32>>;

#[derive(Component)]
struct SpawnedChunk;

#[derive(Component)]
struct GenerateChunk(Task<(IVec2, Chunk)>);

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

#[derive(Default)]
struct ChunkManager {
    spawned_chunks: HashSet<IVec2>,
    loading_chunks: HashSet<IVec2>,
    entities: HashMap<IVec2, Entity>,
}

async fn generate_chunk(chunk_position: IVec2, mut chunk: Chunk) -> (IVec2, Chunk) {
    info!("Start generating chunk {:?}", chunk_position);
    let tilemap_size = CHUNK_SIZE;
    let mut rng = rand::thread_rng();
    // Pick a random tile
    let mut tile_pos = {
        if let Some((x, y)) = chunk
            .indexed_iter()
            .filter_map(|t| match t {
                (pos, Some(_)) => Some(pos),
                _ => None,
            })
            .choose(&mut rng)
        {
            info!("Picked existing tile at {:?} as starting tile", (x, y));
            TilePos {
                x: x.max(1).min(CHUNK_SIZE.x as usize - 1) as u32,
                y: y.max(1).min(CHUNK_SIZE.y as usize - 1) as u32,
            }
        } else {
            TilePos {
                x: rng.gen_range(1..tilemap_size.x),
                y: rng.gen_range(1..tilemap_size.y),
            }
        }
    };
    info!(
        "Start generating chunk {:?} at {:?}",
        chunk_position, tile_pos
    );
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
    let all_tiles = HashSet::from([WATER, GRASS, TALL_GRASS, TREE, FLOWERS, STONE, COAL, IRON]);
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
            error!("No possible tiles found");
            continue;
        }

        let texture_id = *possible.iter().choose(&mut rng).unwrap();
        debug!("Chosen tile: {:?}", texture_id);
        chunk[[tile_pos.x as usize + 1, tile_pos.y as usize + 1]] = Some(texture_id);

        let tile_neighbors = &chunk.slice(s![t_west..=t_east, t_north..=t_south]);
        for ((x, y), tile) in tile_neighbors.indexed_iter() {
            let g_x = tile_pos.x as i32 + x as i32 - 1;
            let g_y = tile_pos.y as i32 + y as i32 - 1;
            if g_x < 0 || g_y < 0 || g_x >= tilemap_size.x as i32 || g_y >= tilemap_size.y as i32 {
                debug!("Skipping neighbor outside of chunk: {:?}", (g_x, g_y));
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
    (chunk_position, chunk.slice_move(s![1..-1, 1..-1]))
}

fn spawn_chunk(
    mut commands: Commands,
    mut chunk_task: Query<(Entity, &mut GenerateChunk)>,
    mut chunk_manager: ResMut<ChunkManager>,
    asset_server: Res<AssetServer>,
) {
    for (chunk_entity, mut task) in &mut chunk_task {
        if let Some((chunk_position, chunk)) = future::block_on(future::poll_once(&mut task.0)) {
            let tilemap_size = TilemapSize {
                x: chunk.ncols() as u32,
                y: chunk.nrows() as u32,
            };

            let mut tile_storage = TileStorage::empty(tilemap_size);
            for ((x, y), tile) in chunk.indexed_iter() {
                let x = x as u32;
                let y = y as u32;
                if let Some(texture_id) = tile {
                    let tile_pos = TilePos { x, y };
                    let tile_entity = commands
                        .spawn()
                        .insert_bundle(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(chunk_entity),
                            texture: TileTexture(*texture_id),
                            ..default()
                        })
                        .id();

                    tile_storage.set(&tile_pos, Some(tile_entity));
                } else {
                    warn!("Tile at {:?} is empty", TilePos { x, y });
                }
            }

            let texture_handle = asset_server.load("textures/terrain.png");

            info!("Adding chunk {:?} to world", chunk_position);
            commands
                .entity(chunk_entity)
                .insert_bundle(TilemapBundle {
                    grid_size: TILE_SIZE.into(),
                    size: CHUNK_SIZE.into(),
                    storage: tile_storage,
                    texture: TilemapTexture(texture_handle),
                    tile_size: TILE_SIZE,
                    transform: Transform::from_translation(Vec3::new(
                        chunk_position.x as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x,
                        chunk_position.y as f32 * CHUNK_SIZE.y as f32 * TILE_SIZE.y,
                        0.0,
                    )),
                    ..default()
                })
                .insert(SpawnedChunk);

            commands.entity(chunk_entity).remove::<GenerateChunk>();
            chunk_manager.loading_chunks.remove(&chunk_position);
            chunk_manager.spawned_chunks.insert(chunk_position);
            chunk_manager.entities.insert(chunk_position, chunk_entity);
        }
    }
}

fn spawn_chunks_around_camera(
    mut commands: Commands,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    mut chunk_manager: ResMut<ChunkManager>,
    terrain_settings: Res<TerrainSettings>,
    chunks_query: Query<(&Transform, &TileStorage), With<SpawnedChunk>>,
    tile_query: Query<&TileTexture>,
) {
    for transform in &camera_query {
        let camera_chunk_pos = camera_pos_to_chunk_pos(&transform.translation().xy());
        let chunk_spawn_radius = terrain_settings.chunk_spawn_radius;
        for y in
            (camera_chunk_pos.y - chunk_spawn_radius)..(camera_chunk_pos.y + chunk_spawn_radius)
        {
            for x in
                (camera_chunk_pos.x - chunk_spawn_radius)..(camera_chunk_pos.x + chunk_spawn_radius)
            {
                if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                    let neighboring_loading_chunks = {
                        let mut any_neighbors = false;
                        for c_x in -1..=1 {
                            for c_y in -1..=1 {
                                any_neighbors = any_neighbors
                                    | chunk_manager
                                        .loading_chunks
                                        .contains(&IVec2::new(x + c_x, y + c_y))
                            }
                        }
                        any_neighbors
                    };
                    if !neighboring_loading_chunks {
                        info!("Spawning chunk {:?}", IVec2::new(x, y));
                        // chunk_manager.spawned_chunks.insert(IVec2::new(x, y));
                        chunk_manager.loading_chunks.insert(IVec2::new(x, y));
                        let mut chunk = Array2::<Option<u32>>::default((
                            CHUNK_SIZE.x as usize + 2,
                            CHUNK_SIZE.y as usize + 2,
                        ));

                        if let Some(left_chunk_entity) =
                            chunk_manager.entities.get(&IVec2::new(x - 1, y))
                        {
                            let (_chunk_transform, tile_storage) = chunks_query
                                .get(*left_chunk_entity)
                                .expect(&format!("Chunk {:?} should exist", (x - 1, y)));
                            for c_y in 0..CHUNK_SIZE.y {
                                if let Some(tile_entity) = tile_storage.get(&TilePos {
                                    x: CHUNK_SIZE.x - 1,
                                    y: c_y,
                                }) {
                                    let tile =
                                        tile_query.get(tile_entity).expect("Tile should exist");
                                    chunk[[0, c_y as usize + 1]] = Some(tile.0);
                                }
                            }
                        }

                        if let Some(right_chunk_entity) =
                            chunk_manager.entities.get(&IVec2::new(x + 1, y))
                        {
                            let (_chunk_transform, tile_storage) = chunks_query
                                .get(*right_chunk_entity)
                                .expect(&format!("Chunk {:?} should exist", (x + 1, y)));
                            for c_y in 0..CHUNK_SIZE.y {
                                if let Some(tile_entity) =
                                    tile_storage.get(&TilePos { x: 0, y: c_y })
                                {
                                    let tile =
                                        tile_query.get(tile_entity).expect("Tile should exist");
                                    chunk[[CHUNK_SIZE.x as usize + 1, c_y as usize + 1]] =
                                        Some(tile.0);
                                }
                            }
                        }

                        if let Some(bottom_chunk_entity) =
                            chunk_manager.entities.get(&IVec2::new(x, y + 1))
                        {
                            let (_chunk_transform, tile_storage) = chunks_query
                                .get(*bottom_chunk_entity)
                                .expect(&format!("Chunk {:?} should exist", (x, y + 1)));
                            for c_x in 0..CHUNK_SIZE.x {
                                if let Some(tile_entity) =
                                    tile_storage.get(&TilePos { x: c_x, y: 0 })
                                {
                                    let tile =
                                        tile_query.get(tile_entity).expect("Tile should exist");
                                    chunk[[c_x as usize + 1, CHUNK_SIZE.y as usize + 1]] =
                                        Some(tile.0);
                                }
                            }
                        }

                        if let Some(top_chunk_entity) =
                            chunk_manager.entities.get(&IVec2::new(x, y - 1))
                        {
                            let (_chunk_transform, tile_storage) = chunks_query
                                .get(*top_chunk_entity)
                                .expect(&format!("Chunk {:?} should exist", (x, y - 1)));
                            for c_x in 0..CHUNK_SIZE.x {
                                if let Some(tile_entity) = tile_storage.get(&TilePos {
                                    x: c_x,
                                    y: CHUNK_SIZE.y - 1,
                                }) {
                                    let tile =
                                        tile_query.get(tile_entity).expect("Tile should exist");
                                    chunk[[c_x as usize + 1, 0]] = Some(tile.0);
                                }
                            }
                        }

                        let thread_pool = AsyncComputeTaskPool::get();
                        let task = thread_pool
                            .spawn(async move { generate_chunk(IVec2::new(x, y), chunk).await });
                        commands.spawn().insert(GenerateChunk(task));
                    }
                }
            }
        }
    }
}

fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let camera_pos = camera_pos.as_ivec2();
    let chunk_size: IVec2 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    let tile_size: IVec2 = IVec2::new(TILE_SIZE.x as i32, TILE_SIZE.y as i32);
    camera_pos / (chunk_size * tile_size)
}

fn despawn_outofrange_chunks(
    mut commands: Commands,
    camera_query: Query<&Transform, With<Player>>,
    chunks_query: Query<(Entity, &Transform), With<SpawnedChunk>>,
    mut chunk_manager: ResMut<ChunkManager>,
    terrain_settings: Res<TerrainSettings>,
) {
    for player_transform in &camera_query {
        for (entity, chunk_transform) in &chunks_query {
            let chunk_pos = chunk_transform.translation.xy();
            let distance = player_transform.translation.xy().distance(chunk_pos);
            let chunk_spawn_radius = terrain_settings.chunk_spawn_radius;
            if distance > (chunk_spawn_radius as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x) * 2 as f32
            {
                let x = (chunk_pos.x as f32 / (CHUNK_SIZE.x as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y as f32 / (CHUNK_SIZE.y as f32 * TILE_SIZE.y)).floor() as i32;

                info!("Despawning chunk {:?} at {:?}", (x, y), chunk_pos);
                chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                chunk_manager.entities.remove(&IVec2::new(x, y));
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

fn debug_ui(
    mut egui_context: ResMut<EguiContext>,
    chunk_task: Query<(Entity, &mut GenerateChunk)>,
    camera_query: Query<&GlobalTransform, With<Camera>>,
    mut terrain_settings: ResMut<TerrainSettings>,
) {
    egui::Window::new("Terrain").show(egui_context.ctx_mut(), |ui| {
        ui.label(format!(
            "Chunks being generated: {}",
            chunk_task.iter().count()
        ));
        ui.label(format!(
            "camera position: chunk {}",
            camera_pos_to_chunk_pos(
                &camera_query
                    .get_single()
                    .expect("There should be a camera")
                    .translation()
                    .xy()
            )
        ));
        ui.add(
            egui::Slider::new(&mut terrain_settings.chunk_spawn_radius, 1..=100)
                .text("chunk spawn radius"),
        );
    });
}
