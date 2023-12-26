mod debug;
mod terrain_generator;

#[cfg(feature = "async")]
use bevy::tasks::{AsyncComputeTaskPool, Task};
#[cfg(feature = "async")]
use futures_lite::future;

use bevy::{
    ecs::system::SystemParam,
    math::Vec3Swizzles,
    prelude::*,
    utils::{HashMap, HashSet},
};

use bevy_ecs_tilemap::prelude::*;
use ndarray::prelude::*;

use crate::{player::Player, types::AppState};

use self::{
    debug::{chunk_gizmos, hovered_tile_gizmo},
    terrain_generator::{NoiseChunkGenerator, TerrainGenerator},
};

pub use self::debug::TerrainDebug;

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
            .insert_resource(TerrainSettings::new(1234567, 5))
            .insert_resource(CursorWorldPos(Vec3::new(-100., -100., 0.)))
            .add_systems(Startup, setup_terrain)
            .add_systems(
                Update,
                (
                    spawn_chunks_around_camera,
                    spawn_generated_chunks,
                    update_cursor_pos,
                    hovered_tile,
                    (chunk_gizmos, hovered_tile_gizmo).run_if(resource_exists::<TerrainDebug>()),
                )
                    .in_set(TerrainSet)
                    .run_if(in_state(AppState::Running)),
            );

        #[cfg(feature = "async")]
        app.add_systems(
            Update,
            spawn_terrain_data
                .in_set(TerrainSet)
                .run_if(in_state(AppState::Running)),
        );
    }
}

// TODO: get rid of this
pub const CHUNK_SIZE: UVec2 = UVec2 { x: 9, y: 9 };
pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16., y: 16. };
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

#[derive(Component)]
pub struct Terrain;

#[derive(Resource, Debug, Reflect)]
struct TerrainSettings {
    seed: u32,
    chunk_spawn_radius: i32,
    #[cfg(feature = "async")]
    enable_async: bool,
}

impl TerrainSettings {
    fn new(seed: u32, chunk_spawn_radius: i32) -> Self {
        Self {
            seed,
            chunk_spawn_radius,
            #[cfg(feature = "async")]
            enable_async: true,
        }
    }
}

#[derive(Component, Debug, Reflect)]
struct Chunk {
    position: IVec2,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ChunkData(Array2<Option<u32>>);

#[cfg(feature = "async")]
#[derive(Component)]
#[component(storage = "SparseSet")]
struct GenerateChunk(Task<ChunkData>);

#[derive(Bundle)]
struct TerrainBundle {
    terrain: Terrain,
    generator: TerrainGenerator,
    name: Name,
    transform: TransformBundle,
    visibility: VisibilityBundle,
}

impl TerrainBundle {
    pub fn new(generator: TerrainGenerator) -> Self {
        Self {
            terrain: Terrain,
            generator,
            name: Name::new("Terrain"),
            transform: TransformBundle::default(),
            visibility: VisibilityBundle::default(),
        }
    }
}

#[derive(Default, Resource, Debug, Reflect)]
pub struct ChunkManager {
    spawned_chunks: HashSet<IVec2>,
    loading_chunks: HashSet<IVec2>,
    pub entities: HashMap<IVec2, Entity>,
}

fn setup_terrain(mut commands: Commands) {
    let chunk_generator = NoiseChunkGenerator::new(1234567);
    let terrain_generator = TerrainGenerator::new(Box::new(chunk_generator));

    commands.spawn(TerrainBundle::new(terrain_generator));
}

fn queue_chunk_generation(
    commands: &mut Commands,
    terrain_entity: Entity,
    chunk_position: IVec2,
    generator: &TerrainGenerator,
    chunk_manager: &mut ChunkManager,
    #[cfg(feature = "async")] enable_async: bool,
) {
    if chunk_manager.spawned_chunks.contains(&chunk_position) {
        return;
    }

    let IVec2 { x, y } = chunk_position;

    let transform = Transform::from_translation(Vec3::new(
        x as f32 * CHUNK_SIZE.x as f32,
        y as f32 * CHUNK_SIZE.y as f32,
        0.0,
    ));

    #[cfg(feature = "async")]
    if enable_async {
        let thread_pool = AsyncComputeTaskPool::get();
        let generator_clone = generator.clone();
        let task = thread_pool.spawn(async move { generator_clone.generate_chunk(chunk_position) });

        commands.entity(terrain_entity).with_children(|parent| {
            parent.spawn((
                GenerateChunk(task),
                Chunk {
                    position: chunk_position,
                },
                TransformBundle::from_transform(transform),
                VisibilityBundle::default(),
            ));
        });
        return;
    }

    #[cfg(not(feature = "async"))]
    {
        let chunk_data = generator.generate_chunk(chunk_position);
        commands.entity(terrain_entity).with_children(|parent| {
            parent.spawn((
                chunk_data,
                Chunk {
                    position: chunk_position,
                },
                TransformBundle::from_transform(transform),
                VisibilityBundle::default(),
            ));
        });
    }
}

#[cfg(feature = "async")]
fn spawn_terrain_data(
    mut commands: Commands,
    mut generate_terrain_query: Query<(Entity, &mut GenerateChunk)>,
) {
    for (terrain_entity, mut generate_terrain) in &mut generate_terrain_query {
        if let Some(terrain_data) = future::block_on(future::poll_once(&mut generate_terrain.0)) {
            commands
                .entity(terrain_entity)
                .remove::<GenerateChunk>()
                .insert(terrain_data);
        }
    }
}

fn spawn_generated_chunks(
    mut commands: Commands,
    mut chunk_task: Query<(Entity, &Chunk, &ChunkData), Added<ChunkData>>,
    mut chunk_manager: ResMut<ChunkManager>,
    asset_server: Res<AssetServer>,
) {
    let map_type = TilemapType::Square;
    for (chunk_entity, chunk, chunk_data) in &mut chunk_task {
        let chunk_data = chunk_data.0.view();
        let tilemap_size = TilemapSize {
            x: chunk_data.ncols() as u32,
            y: chunk_data.nrows() as u32,
        };

        let map_transform = Transform::from_translation(Vec3::new(
            -(0.5 * CHUNK_SIZE.x as f32) + 0.5,
            -(0.5 * CHUNK_SIZE.y as f32) + 0.5,
            0.0,
        ));

        let tilemap_entity = commands.spawn_empty().id();
        let mut tile_storage = TileStorage::empty(tilemap_size);
        for ((x, y), tile) in chunk_data.indexed_iter() {
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
            transform: map_transform.with_scale(Vec3::new(1. / TILE_SIZE.x, 1. / TILE_SIZE.y, 1.)),
            map_type,
            ..default()
        });

        debug!(position = ?chunk.position, "Adding chunk to world");
        commands
            .entity(chunk_entity)
            .insert((Name::new(format!(
                "Chunk {},{}",
                chunk.position.x, chunk.position.y
            )),))
            .add_child(tilemap_entity);

        chunk_manager.loading_chunks.remove(&chunk.position);
        chunk_manager.spawned_chunks.insert(chunk.position);
        chunk_manager.entities.insert(chunk.position, chunk_entity);
    }
}

fn spawn_chunks_around_camera(
    camera_query: Query<&GlobalTransform, With<Camera>>,
    terrain_settings: Res<TerrainSettings>,
    mut terrain_params: TerrainParams,
    terrain_entity_query: Query<Entity, With<Terrain>>,
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
                let chunk_position = IVec2::new(x, y);
                let terrain_entity = terrain_entity_query.single();
                terrain_params.spawn_chunk(terrain_entity, chunk_position)
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
    terrain_params: TerrainParams,
    player_query: Query<Entity, With<Player>>,
) {
    for hovered_tile in &hovered_tile_query {
        commands.entity(hovered_tile).remove::<HoveredTile>();
    }

    let Some(player_entity) = player_query.iter().next() else {
        return;
    };

    let cursor_pos = cursor_world_pos.0;

    if let Some(tile_entity) = terrain_params.tile_entity_at_global_pos(cursor_pos.xy()) {
        commands.entity(player_entity).insert(HoveredTile {
            entity: tile_entity,
            tile_center: cursor_pos.xy().round(),
        });
    }
}

fn terrain_pos_to_chunk_id(terrain_pos: Vec2) -> IVec2 {
    IVec2::new(
        ((terrain_pos.x + CHUNK_SIZE.x as f32 * 0.5) / CHUNK_SIZE.x as f32).floor() as i32,
        ((terrain_pos.y + CHUNK_SIZE.y as f32 * 0.5) / CHUNK_SIZE.y as f32).floor() as i32,
    )
}

#[derive(SystemParam)]
pub struct TerrainParams<'w, 's> {
    commands: Commands<'w, 's>,
    tiles: Query<'w, 's, &'static TileTextureIndex>,
    terrain_query: Query<'w, 's, (&'static GlobalTransform, &'static Terrain)>,
    generator_query: Query<'w, 's, &'static TerrainGenerator>,
    chunk_manager: ResMut<'w, ChunkManager>,
    children_query: Query<'w, 's, &'static Children>,
    tilemap_query: Query<
        'w,
        's,
        (
            &'static TileStorage,
            &'static TilemapGridSize,
            &'static GlobalTransform,
        ),
    >,
    terrain_settings: Res<'w, TerrainSettings>,
}

impl<'w, 's> TerrainParams<'w, 's> {
    pub fn spawn_chunk(&mut self, terrain_entity: Entity, chunk_position: IVec2) {
        let generator = self
            .generator_query
            .get(terrain_entity)
            .expect("Terrain entity not found");

        queue_chunk_generation(
            &mut self.commands,
            terrain_entity,
            chunk_position,
            generator,
            &mut self.chunk_manager,
            #[cfg(feature = "async")]
            self.terrain_settings.enable_async,
        );
    }

    /// Retrieves the tile entity at a given world position.
    ///
    /// # Arguments
    /// * `world_position` - The position in the world.
    ///
    /// # Returns
    /// An `Option<Entity>` representing the tile entity at the specified world position.
    /// Returns `None` if no tile entity is found at that position.
    pub fn tile_entity_at_global_pos(&self, world_position: Vec2) -> Option<Entity> {
        // Retrieve the global transform of the terrain to convert world positions
        let terrain_transform = self.terrain_query.single().0;

        // Transform the world position to the terrain's local coordinate system
        let terrain_local_position = terrain_transform
            .compute_matrix()
            .inverse()
            .transform_point3(world_position.extend(0.0))
            .truncate();

        // Calculate the coordinates of the chunk based on the local position within the terrain
        let chunk_coordinates = terrain_pos_to_chunk_id(terrain_local_position);

        // Retrieve the tilemap entity and tile storage for the chunk that contains the given position
        self.chunk_manager
            .entities
            .get(&chunk_coordinates)
            .and_then(|chunk_entity| self.children_query.get(*chunk_entity).ok())
            .and_then(|children| {
                // Find the tilemap entity among the children of the chunk entity
                children
                    .iter()
                    .find_map(|child| self.tilemap_query.get(*child).ok())
            })
            .and_then(|(tile_storage, tilemap_grid_size, tilemap_transform)| {
                // Convert the world position to the local position within the tilemap
                let local_tilemap_position = tilemap_transform
                    .compute_matrix()
                    .inverse()
                    .transform_point3(world_position.extend(0.0))
                    .truncate();

                // Calculate the tile position from the local position within the tilemap
                TilePos::from_world_pos(
                    &local_tilemap_position,
                    &CHUNK_SIZE.into(),
                    tilemap_grid_size,
                    &TilemapType::Square,
                )
                // Retrieve the tile entity from the tile storage
                .and_then(|tile_pos| tile_storage.get(&tile_pos))
            })
    }

    pub fn tile_texture_index(&self, tile_entity: Entity) -> Option<TileTextureIndex> {
        self.tiles.get(tile_entity).ok().copied()
    }
}

#[cfg(test)]
mod test {
    use super::*;

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
