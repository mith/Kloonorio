use rand::seq::SliceRandom;

use rand::SeedableRng;

use std::hash::{BuildHasher, Hasher};

use ahash::{AHasher, RandomState};
use bevy::{
    ecs::system::SystemParam,
    math::{Vec3Swizzles, Vec4Swizzles},
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
    transform,
    utils::{HashMap, HashSet},
};
use bevy_ecs_tilemap::prelude::*;
use futures_lite::future;
use ndarray::prelude::*;

use fast_poisson::Poisson2D;
use noise::{NoiseFn, OpenSimplex, ScalePoint, Seedable, SuperSimplex, Turbulence};
use rand_xoshiro::Xoshiro256StarStar;

use crate::{player::Player, types::AppState};

#[derive(SystemSet, Hash, Debug, PartialEq, Eq, Clone)]
pub struct TerrainSet;

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .register_type::<TerrainSettings>()
            .register_type::<ChunkManager>()
            .register_type::<CursorWorldPos>()
            .register_type::<HoveredTile>()
            .insert_resource(ChunkManager::default())
            .insert_resource(TerrainSettings {
                seed: 1234567,
                chunk_spawn_radius: 5,
            })
            .insert_resource(CursorWorldPos(Vec3::new(-100., -100., 0.)))
            .add_systems(Startup, setup_terrain)
            .add_systems(
                Update,
                (
                    spawn_chunks_around_camera,
                    spawn_chunk_tilemap,
                    update_cursor_pos,
                    hovered_tile,
                    (chunk_gizmos, hovered_tile_gizmo).run_if(resource_exists::<TerrainDebug>()),
                )
                    .in_set(TerrainSet)
                    .run_if(in_state(AppState::Running)),
            );
    }
}

pub const CHUNK_SIZE: UVec2 = UVec2 { x: 9, y: 9 };
pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16., y: 16. };

#[derive(Component)]
pub struct Terrain;

#[derive(Resource, Debug, Reflect)]
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

#[derive(Default, Resource, Debug, Reflect)]
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

fn setup_terrain(mut commands: Commands) {
    commands.spawn((
        Name::new("Terrain"),
        Terrain,
        TransformBundle::default(),
        VisibilityBundle::default(),
    ));
}

#[derive(Component)]
struct TileLabel;

fn spawn_chunk_tilemap(
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
                -(0.5 * CHUNK_SIZE.x as f32) + 0.5,
                -(0.5 * CHUNK_SIZE.y as f32) + 0.5,
                0.0,
            ));

            let tilemap_entity = commands.spawn_empty().id();
            let mut tile_storage = TileStorage::empty(tilemap_size);
            for ((x, y), tile) in chunk.indexed_iter() {
                let x = x as u32;
                let y = y as u32;
                if let Some(texture_id) = tile {
                    let tile_pos = TilePos { x, y };
                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(tilemap_entity),
                            texture_index: TileTextureIndex(*texture_id),

                            ..default()
                        })
                        .id();

                    commands.entity(tilemap_entity).add_child(tile_entity);
                    tile_storage.set(&tile_pos, tile_entity);
                } else {
                    warn!("Tile at {:?} is empty", TilePos { x, y });
                }
            }

            let texture_handle = asset_server.load("textures/terrain.png");

            commands.entity(tilemap_entity).insert(TilemapBundle {
                grid_size: TILE_SIZE.into(),
                size: CHUNK_SIZE.into(),
                storage: tile_storage,
                texture: TilemapTexture::Single(texture_handle),
                tile_size: TILE_SIZE,
                transform: map_transform.with_scale(Vec3::new(
                    1. / TILE_SIZE.x,
                    1. / TILE_SIZE.y,
                    1.,
                )),
                map_type,
                ..default()
            });

            debug!(position = ?chunk_position, "Adding chunk to world");
            commands
                .entity(chunk_entity)
                .insert((
                    Name::new(format!("Chunk {},{}", chunk_position.x, chunk_position.y)),
                    SpawnedChunk,
                ))
                .add_child(tilemap_entity);

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
    terrain_query: Query<Entity, With<Terrain>>,
) {
    for transform in &camera_query {
        let camera_chunk_pos = terrain_pos_to_chunk_id(transform.translation().xy());
        let chunk_spawn_radius = terrain_settings.chunk_spawn_radius;
        for y in
            (camera_chunk_pos.y - chunk_spawn_radius)..(camera_chunk_pos.y + chunk_spawn_radius)
        {
            for x in
                (camera_chunk_pos.x - chunk_spawn_radius)..(camera_chunk_pos.x + chunk_spawn_radius)
            {
                let terrain_entity = terrain_query.single();
                if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                    let thread_pool = AsyncComputeTaskPool::get();
                    let seed = terrain_settings.seed;
                    let task = thread_pool
                        .spawn(async move { generate_chunk_noise(seed, IVec2::new(x, y)).await });
                    commands.entity(terrain_entity).with_children(|parent| {
                        parent.spawn((
                            GenerateChunk(task),
                            TransformBundle::from_transform(Transform::from_translation(
                                Vec3::new(
                                    x as f32 * CHUNK_SIZE.x as f32,
                                    y as f32 * CHUNK_SIZE.y as f32,
                                    0.0,
                                ),
                            )),
                            VisibilityBundle::default(),
                        ));
                    });
                }
            }
        }
    }
}

#[derive(Debug, Default, Resource, Reflect)]
pub struct CursorWorldPos(pub Vec3);

fn update_cursor_pos(
    window: Query<&Window>,
    camera_query: Query<(&GlobalTransform, &Camera)>,
    mut cursor_pos: ResMut<CursorWorldPos>,
) {
    let Some(cursor_position) = window.single().cursor_position() else {
        return;
    };
    let (camera_transform, camera) = camera_query.single();
    let Some(point) = camera.viewport_to_world_2d(camera_transform, cursor_position) else {
        return;
    };

    *cursor_pos = CursorWorldPos(point.extend(0.));
}

#[derive(Resource, Default)]
pub struct TerrainDebug;

fn chunk_gizmos(mut gizmos: Gizmos, chunk_query: Query<&GlobalTransform, With<SpawnedChunk>>) {
    for transform in chunk_query.iter() {
        let half_width = CHUNK_SIZE.x as f32 / 2.0;
        let half_height = CHUNK_SIZE.y as f32 / 2.0;
        let center = transform.translation().truncate();

        // Draw chunk borders
        gizmos.line_2d(
            center + Vec2::new(-half_width, -half_height),
            center + Vec2::new(half_width, -half_height),
            Color::WHITE,
        );
        gizmos.line_2d(
            center + Vec2::new(half_width, -half_height),
            center + Vec2::new(half_width, half_height),
            Color::WHITE,
        );
        gizmos.line_2d(
            center + Vec2::new(half_width, half_height),
            center + Vec2::new(-half_width, half_height),
            Color::WHITE,
        );
        gizmos.line_2d(
            center + Vec2::new(-half_width, half_height),
            center + Vec2::new(-half_width, -half_height),
            Color::WHITE,
        );

        gizmos.rect_2d(center, 0., Vec2::ONE, Color::WHITE);
    }
}

#[derive(Component)]
struct HighlightedLabel;

#[derive(Component, Debug, Reflect)]
#[component(storage = "SparseSet")]
pub struct HoveredTile {
    pub entity: Entity,
    pub tile_center: Vec2,
}

pub fn hovered_tile(
    mut commands: Commands,
    cursor_world_pos: Res<CursorWorldPos>,
    hovered_tile_query: Query<Entity, With<HoveredTile>>,
    children_query: Query<&Children>,
    tilemap_query: Query<(&TileStorage, &GlobalTransform)>,
    chunk_manager: Res<ChunkManager>,
    player_query: Query<Entity, With<Player>>,
) {
    for hovered_tile in &hovered_tile_query {
        commands.entity(hovered_tile).remove::<HoveredTile>();
    }

    let Some(player_entity) = player_query.iter().next() else {
        return;
    };

    let cursor_pos = cursor_world_pos.0;
    let chunk_id = terrain_pos_to_chunk_id(cursor_pos.xy());

    if let Some((_tilemap_entity, tile_storage, tilemap_transform)) = chunk_manager
        .entities
        .get(&chunk_id)
        .and_then(|chunk_entity| {
            children_query.get(*chunk_entity).ok().and_then(|children| {
                children.iter().find_map(|child| {
                    tilemap_query
                        .get(*child)
                        .ok()
                        .map(|(storage, transform)| (*child, storage, transform))
                })
            })
        })
    {
        // Convert the cursor's global position to the chunk's local coordinate system
        let local_cursor_pos = tilemap_transform
            .compute_matrix()
            .inverse()
            .transform_point3(cursor_pos)
            .truncate();

        // Calculate the tile position from the local cursor position
        let tile_x =
            ((local_cursor_pos.x + CHUNK_SIZE.x as f32 / 2.0) / TILE_SIZE.x).floor() as u32;
        let tile_y =
            ((local_cursor_pos.y + CHUNK_SIZE.y as f32 / 2.0) / TILE_SIZE.y).floor() as u32;
        let tile_pos = TilePos {
            x: tile_x,
            y: tile_y,
        };

        // Retrieve the tile entity
        if let Some(tile_entity) = tile_storage.checked_get(&tile_pos) {
            let tile_center = tile_pos.center_in_world(&TILE_SIZE.into(), &TilemapType::Square);
            let tile_center =
                tilemap_transform.compute_matrix() * tile_center.extend(0.0).extend(1.0);
            commands.entity(player_entity).insert(HoveredTile {
                entity: tile_entity,
                tile_center: tile_center.xy(),
            });
        }
    }
}

fn terrain_pos_to_chunk_id(terrain_pos: Vec2) -> IVec2 {
    IVec2::new(
        ((terrain_pos.x + CHUNK_SIZE.x as f32 * 0.5) / CHUNK_SIZE.x as f32).floor() as i32,
        ((terrain_pos.y + CHUNK_SIZE.y as f32 * 0.5) / CHUNK_SIZE.y as f32).floor() as i32,
    )
}

fn hovered_tile_gizmo(mut gizmos: Gizmos, hovered_tile_query: Query<&HoveredTile>) {
    for hovered_tile in &hovered_tile_query {
        let tile_center = hovered_tile.tile_center;
        gizmos.rect_2d(tile_center, 0., Vec2::ONE, Color::YELLOW);
    }
}

#[derive(SystemParam)]
pub struct TerrainParams<'w, 's> {
    tiles: Query<'w, 's, &'static TileTextureIndex>,
    terrain_query: Query<'w, 's, &'static GlobalTransform, With<Terrain>>,
    chunk_manager: Res<'w, ChunkManager>,
    children_query: Query<'w, 's, &'static Children>,
    tilemap_query: Query<'w, 's, (&'static TileStorage, &'static GlobalTransform)>,
}

impl<'w, 's> TerrainParams<'w, 's> {
    pub fn tile_entity_at_global_pos(&self, global_pos: Vec2) -> Option<Entity> {
        // Transform the global position to the terrain's local coordinate system
        let terrain_transform = self.terrain_query.single();
        let local_pos = terrain_transform
            .compute_matrix()
            .inverse()
            .transform_point3(global_pos.extend(0.))
            .truncate();
        let chunk_id = terrain_pos_to_chunk_id(local_pos);

        // Calculate the terrain tile position from the local position
        let terrain_tile_pos = local_pos.as_ivec2();

        // Calculate the tilemap position from the terrain tile position
        // Terrain tile position (0, 0) is at the center of the tilemap
        // of the chunk at chunk position (0, 0)

        let tilemap_pos = terrain_tile_pos - (chunk_id * CHUNK_SIZE.as_ivec2());

        // Retrieve the tilemap entity and tile storage for the chunk
        if let Some(tile_storage) =
            self.chunk_manager
                .entities
                .get(&chunk_id)
                .and_then(|chunk_entity| {
                    self.children_query
                        .get(*chunk_entity)
                        .ok()
                        .and_then(|children| {
                            children.iter().find_map(|child| {
                                self.tilemap_query
                                    .get(*child)
                                    .ok()
                                    .map(|(storage, _transform)| storage)
                            })
                        })
                })
        {
            // Convert the tilemap position to a TilePos
            let tile_x = tilemap_pos.x as u32;
            let tile_y = tilemap_pos.y as u32;
            let tile_pos = TilePos {
                x: tile_x,
                y: tile_y,
            };

            // Retrieve the tile entity from the tile storage
            return tile_storage.get(&tile_pos);
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

    #[tokio::test]
    async fn generate_chunk_is_reproducible() {
        let seed = 123456789;
        let position = IVec2::new(100, 100);
        let chunk_a = generate_chunk_noise(seed, position).await;
        let chunk_b = generate_chunk_noise(seed, position).await;
        assert_eq!(chunk_a, chunk_b);
    }

    #[test]
    fn test_global_pos_to_chunk_id() {
        let test_cases :Vec<(IVec2, Vec2)> =
            // generate IVec2 from (-1, -1) to (1, 1)
            (-1..=1).flat_map(|x| (-1..=1).map(move |y| IVec2::new(x, y)))
            // generate global position by multiplying chunk size by 0.6
            .map(|chunk_id| {
                (chunk_id, Vec2::new(CHUNK_SIZE.x as f32 * 0.6, CHUNK_SIZE.y as f32 * 0.6) * chunk_id.as_vec2())
            })
            .collect();

        for (expected_chunk_id, global_pos) in test_cases {
            let chunk_id = terrain_pos_to_chunk_id(global_pos);
            assert_eq!(
                chunk_id, expected_chunk_id,
                "global_pos_to_chunk_id failed for global position {:?}: expected {:?}, got {:?}",
                global_pos, expected_chunk_id, chunk_id
            );
        }
    }
}
