use iyes_loopless::prelude::ConditionSet;

use rand::seq::SliceRandom;

use rand::SeedableRng;

use std::hash::{BuildHasher, Hasher};

use ahash::{AHasher, RandomState};
use bevy::{
    ecs::system::SystemParam,
    math::{Vec3Swizzles, Vec4Swizzles},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
    utils::{HashMap, HashSet},
};
use bevy_ecs_tilemap::prelude::*;
use futures_lite::future;
use ndarray::prelude::*;

use fast_poisson::Poisson2D;
use noise::{NoiseFn, OpenSimplex, ScalePoint, Seedable, SuperSimplex, Turbulence};
use rand_xoshiro::Xoshiro256StarStar;

use crate::types::AppState;
use crate::types::Player;

#[derive(SystemLabel)]
pub struct TerrainStage;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(TilemapPlugin)
            .insert_resource(ChunkManager::default())
            .insert_resource(TerrainSettings {
                seed: 1234567,
                chunk_spawn_radius: 5,
            })
            .insert_resource(CursorPos(Vec3::new(-100., -100., 0.)))
            .add_system_set(
                ConditionSet::new()
                    .run_in_state(AppState::Running)
                    .label(TerrainStage)
                    .with_system(spawn_chunks_around_camera)
                    .with_system(spawn_chunk)
                    // .with_system(despawn_outofrange_chunks)
                    // .with_system(debug_ui)
                    .with_system(update_cursor_pos)
                    // .with_system(hover_info_ui)
                    .with_system(hovered_tile)
                    // .with_system(highlight_tile_labels)
                    .into(),
            );
    }
}

pub const CHUNK_SIZE: UVec2 = UVec2 { x: 8, y: 8 };
pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16., y: 16. };

#[derive(Resource)]
struct TerrainSettings {
    seed: u32,
    chunk_spawn_radius: i32,
}

type Chunk = Array2<Option<u32>>;

#[derive(Component)]
pub struct SpawnedChunk;

#[derive(Component)]
#[component(storage = "SparseSet")]
struct GenerateChunk(Task<(IVec2, Chunk)>);

pub const GROUND: u32 = 0;
pub const WATER: u32 = 1;
pub const GRASS: u32 = 2;
pub const TALL_GRASS: u32 = 3;
pub const DEEP_WATER: u32 = 4;
pub const TREE: u32 = 5;
pub const _FLOWERS: u32 = 6;
pub const STONE: u32 = 7;
pub const COAL: u32 = 8;
pub const IRON: u32 = 9;

#[derive(Default, Resource)]
pub struct ChunkManager {
    spawned_chunks: HashSet<IVec2>,
    loading_chunks: HashSet<IVec2>,
    pub entities: HashMap<IVec2, Entity>,
}

#[derive(Debug)]
struct RadiusNoise {
    location: [f64; 2],
    radius: f64,
}

impl NoiseFn<f64, 2> for RadiusNoise {
    /// Return 1. if the point is within the radius, 0. otherwise
    fn get(&self, point: [f64; 2]) -> f64 {
        let dist = (point[0] - self.location[0]).powi(2) + (point[1] - self.location[1]).powi(2);
        if dist < self.radius.powi(2) {
            1.
        } else {
            0.
        }
    }
}

type TileType = u32;

struct Region {
    ores: Vec<(u32, Turbulence<RadiusNoise, OpenSimplex>)>,
}

async fn generate_region(seed: u32, region_location: IVec2) -> Region {
    let useed = seed as u64;
    let mut hasher: AHasher = RandomState::with_seeds(
        useed,
        useed.swap_bytes(),
        useed.count_ones() as u64,
        useed.rotate_left(32),
    )
    .build_hasher();
    hasher.write_i32(region_location.x);
    hasher.write_i32(region_location.y);
    let region_seed = hasher.finish();

    // Generate a list of ore locations for the region
    let ore_locations = Poisson2D::new()
        .with_dimensions(
            [(CHUNK_SIZE.x * 10) as f64, (CHUNK_SIZE.y * 10) as f64],
            30.,
        )
        .with_seed(region_seed)
        .iter()
        .take(10)
        .collect::<Vec<_>>();

    let ore_noise = ore_locations
        .iter()
        .map(|&location| RadiusNoise {
            location,
            radius: 5.,
        })
        .map(|noise| {
            Turbulence::<_, OpenSimplex>::new(noise)
                .set_seed(seed + 11)
                .set_frequency(0.1)
                .set_power(10.)
        });

    let mut rng = Xoshiro256StarStar::seed_from_u64(region_seed);
    let ore_types = ore_locations.iter().map(|_| {
        let ore_types = [(COAL, 2), (IRON, 2), (STONE, 1)];

        let ore_type = ore_types
            .choose_weighted(&mut rng, |item| item.1)
            .unwrap()
            .0;
        ore_type
    });

    Region {
        ores: ore_types.into_iter().zip(ore_noise).collect::<Vec<_>>(),
    }
}

async fn generate_chunk_noise(seed: u32, chunk_position: IVec2) -> (IVec2, Chunk) {
    let mut chunk =
        Array2::<Option<TileType>>::default((CHUNK_SIZE.x as usize, CHUNK_SIZE.y as usize));

    let open_simplex = SuperSimplex::new(seed);
    let scale_point = ScalePoint::new(open_simplex).set_scale(0.005);
    let turbulence = Turbulence::<_, SuperSimplex>::new(scale_point)
        .set_seed(seed + 9)
        .set_frequency(0.001)
        .set_power(100.);
    let turbulence_2 = Turbulence::<_, OpenSimplex>::new(turbulence)
        .set_seed(seed + 10)
        .set_frequency(0.1)
        .set_power(10.)
        .set_roughness(103);
    for ((x, y), tile) in chunk.indexed_iter_mut() {
        let noise = turbulence_2.get([
            (chunk_position.x * CHUNK_SIZE.x as i32 + x as i32).into(),
            (chunk_position.y * CHUNK_SIZE.y as i32 + y as i32).into(),
        ]);
        if noise > 0.4 {
            *tile = Some(TREE);
        } else if noise > 0.2 {
            *tile = Some(TALL_GRASS);
        } else if noise > -0.1 {
            *tile = Some(GRASS);
        } else if noise > -0.3 {
            *tile = Some(GROUND);
        } else if noise > -0.4 {
            *tile = Some(WATER);
        } else {
            *tile = Some(DEEP_WATER);
        }
    }

    let region_location = chunk_position / 10 * 10;
    let region = generate_region(seed, region_location).await;

    for ((x, y), tile) in chunk.indexed_iter_mut() {
        let ore_type = region.ores.iter().fold(None, |acc, (ore_type, noise)| {
            let amount = noise.get([
                ((chunk_position.x - region_location.x) * CHUNK_SIZE.x as i32 + x as i32).into(),
                ((chunk_position.y - region_location.y) * CHUNK_SIZE.y as i32 + y as i32).into(),
            ]);
            if amount > 0. {
                Some(*ore_type)
            } else {
                acc
            }
        });
        if ore_type.is_some() && !matches!(tile, Some(WATER) | Some(DEEP_WATER)) {
            *tile = ore_type;
        }
    }

    (chunk_position, chunk)
}

#[derive(Component)]
struct TileLabel;

fn spawn_chunk(
    mut commands: Commands,
    mut chunk_task: Query<(Entity, &mut GenerateChunk)>,
    mut chunk_manager: ResMut<ChunkManager>,
    asset_server: Res<AssetServer>,
) {
    let map_type = TilemapType::Square;
    for (chunk_entity, mut task) in &mut chunk_task {
        if let Some((chunk_position, chunk)) = future::block_on(future::poll_once(&mut task.0)) {
            let tilemap_size = TilemapSize {
                x: chunk.ncols() as u32,
                y: chunk.nrows() as u32,
            };

            let map_transform = Transform::from_translation(Vec3::new(
                chunk_position.x as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x,
                chunk_position.y as f32 * CHUNK_SIZE.y as f32 * TILE_SIZE.y,
                0.0,
            ));

            let mut tile_storage = TileStorage::empty(tilemap_size);
            for ((x, y), tile) in chunk.indexed_iter() {
                let x = x as u32;
                let y = y as u32;
                if let Some(texture_id) = tile {
                    let tile_pos = TilePos { x, y };
                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(chunk_entity),
                            texture_index: TileTextureIndex(*texture_id),
                            ..default()
                        })
                        .id();

                    // let tile_center = tile_pos
                    //     .center_in_world(&TILE_SIZE.into(), &map_type)
                    //     .extend(1.0);
                    // let transform = Transform::from_translation(tile_center);
                    // commands
                    //     .entity(tile_entity)
                    //     .insert_bundle(SpriteBundle {
                    //         texture: asset_server.load("textures/tile.png"),
                    //         transform,
                    //         ..default()
                    //     });
                    //     .insert(TileLabel);

                    commands.entity(chunk_entity).add_child(tile_entity);
                    tile_storage.set(&tile_pos, tile_entity);
                } else {
                    warn!("Tile at {:?} is empty", TilePos { x, y });
                }
            }

            let texture_handle = asset_server.load("textures/terrain.png");

            debug!("Adding chunk {:?} to world", chunk_position);
            commands.entity(chunk_entity).insert((
                TilemapBundle {
                    grid_size: TILE_SIZE.into(),
                    size: CHUNK_SIZE.into(),
                    storage: tile_storage,
                    texture: TilemapTexture::Single(texture_handle),
                    tile_size: TILE_SIZE,
                    transform: map_transform,
                    map_type,
                    ..default()
                },
                SpawnedChunk,
            ));

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
    chunk_manager: ResMut<ChunkManager>,
    terrain_settings: Res<TerrainSettings>,
) {
    for transform in &camera_query {
        let camera_chunk_pos = global_pos_to_chunk_pos(&transform.translation().xy());
        let chunk_spawn_radius = terrain_settings.chunk_spawn_radius;
        for y in
            (camera_chunk_pos.y - chunk_spawn_radius)..(camera_chunk_pos.y + chunk_spawn_radius)
        {
            for x in
                (camera_chunk_pos.x - chunk_spawn_radius)..(camera_chunk_pos.x + chunk_spawn_radius)
            {
                if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                    let thread_pool = AsyncComputeTaskPool::get();
                    let seed = terrain_settings.seed;
                    let task = thread_pool
                        .spawn(async move { generate_chunk_noise(seed, IVec2::new(x, y)).await });
                    commands.spawn(GenerateChunk(task));
                }
            }
        }
    }
}

pub fn global_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let camera_pos = camera_pos.as_ivec2();
    let chunk_size: IVec2 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    let tile_size: IVec2 = IVec2::new(TILE_SIZE.x as i32, TILE_SIZE.y as i32);
    camera_pos / (chunk_size * tile_size)
}

pub fn cursor_pos_in_world(
    windows: &Windows,
    cursor_pos: Vec2,
    cam_t: &Transform,
    cam: &Camera,
) -> Vec3 {
    let window = windows.primary();

    let window_size = Vec2::new(window.width(), window.height());

    // Convert screen position [0..resolution] to ndc [-1..1]
    // (ndc = normalized device coordinates)
    let ndc_to_world = cam_t.compute_matrix() * cam.projection_matrix().inverse();
    let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
    ndc_to_world.project_point3(ndc.extend(0.0))
}

#[derive(Default, Resource)]
pub struct CursorPos(pub Vec3);

fn update_cursor_pos(
    windows: Res<Windows>,
    camera_query: Query<(&GlobalTransform, &Camera)>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
    player_query: Query<&Transform, With<Player>>,
) {
    if let Some(cursor_moved) = cursor_moved_events.iter().last() {
        for (_cam_t, cam) in camera_query.iter() {
            let player_transform = player_query.single();
            *cursor_pos = CursorPos(cursor_pos_in_world(
                &windows,
                cursor_moved.position,
                player_transform,
                cam,
            ));
        }
    }
}

#[derive(Component)]
struct HighlightedLabel;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HoveredTile {
    pub entity: Entity,
    pub tile_center: Vec2,
}

pub fn hovered_tile(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    hovered_tile_query: Query<Entity, With<HoveredTile>>,
    chunks_query: Query<
        (
            &Transform,
            &TileStorage,
            &TilemapSize,
            &TilemapGridSize,
            &TilemapType,
        ),
        With<SpawnedChunk>,
    >,
    player_query: Query<Entity, With<Player>>,
) {
    if player_query.is_empty() {
        return;
    }
    let player_entity = player_query.single();

    for hovered_tile in &hovered_tile_query {
        commands.entity(hovered_tile).remove::<HoveredTile>();
    }
    let cursor_pos = cursor_pos.0;
    for (chunk_transform, tile_storage, chunk_size, grid_size, map_type) in &chunks_query {
        let cursor_in_chunk_pos: Vec2 = {
            // Extend the cursor_pos vec3 by 1.0
            let cursor_pos = Vec4::from((cursor_pos, 1.));
            let cursor_in_chunk_pos = chunk_transform.compute_matrix().inverse() * cursor_pos;
            cursor_in_chunk_pos.xy()
        };

        if let Some(tile_pos) =
            TilePos::from_world_pos(&cursor_in_chunk_pos, chunk_size, grid_size, map_type)
        {
            if let Some(tile_entity) = tile_storage.get(&tile_pos) {
                let tile_center = tile_pos.center_in_world(&TILE_SIZE.into(), map_type);
                commands.entity(player_entity).insert(HoveredTile {
                    entity: tile_entity,
                    tile_center: chunk_transform.translation.xy() + tile_center,
                });
            }
        }
    }
}

#[derive(SystemParam)]
pub struct Terrain<'w, 's> {
    chunks: Query<
        'w,
        's,
        (
            &'static Transform,
            &'static TileStorage,
            &'static TilemapSize,
            &'static TilemapGridSize,
            &'static TilemapType,
        ),
        With<SpawnedChunk>,
    >,
    tiles: Query<'w, 's, &'static TileTextureIndex>,
}

impl<'w, 's> Terrain<'w, 's> {
    pub fn tile_entity_at_point(&self, point: Vec2) -> Option<Entity> {
        for (chunk_transform, tile_storage, chunk_size, grid_size, map_type) in &self.chunks {
            let point_in_chunk_pos: Vec2 = {
                // Extend the cursor_pos vec3 by 1.0
                let cursor_pos = Vec4::from((point, 0., 1.));
                let cursor_in_chunk_pos = chunk_transform.compute_matrix().inverse() * cursor_pos;
                cursor_in_chunk_pos.xy()
            };

            if let Some(tile_pos) =
                TilePos::from_world_pos(&point_in_chunk_pos, chunk_size, grid_size, map_type)
            {
                if let Some(tile_entity) = tile_storage.get(&tile_pos) {
                    return Some(tile_entity);
                }
            }
        }

        None
    }

    pub fn tile_texture_index(&self, tile_entity: Entity) -> Option<TileTextureIndex> {
        self.tiles.get(tile_entity).ok().copied()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn generate_chunk_is_reproducible() {
        let seed = 123456789;
        let position = IVec2::new(100, 100);
        let chunk_a = generate_chunk_noise(seed, position).await;
        let chunk_b = generate_chunk_noise(seed, position).await;
        assert_eq!(chunk_a, chunk_b);
    }
}
