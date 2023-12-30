mod debug;
pub mod terrain_generator;

#[cfg(feature = "async")]
use bevy::tasks::{AsyncComputeTaskPool, Task};
#[cfg(feature = "async")]
use futures_lite::future;

use bevy::{ecs::system::SystemParam, math::Vec3Swizzles, prelude::*, utils::HashMap};

use bevy_ecs_tilemap::prelude::*;
use ndarray::prelude::*;

use crate::{player::Player, types::AppState};

use self::{
    debug::{chunk_gizmos, hovered_tile_gizmo},
    terrain_generator::{FlatChunkGenerator, TerrainGenerator},
};

pub use self::debug::TerrainDebug;

#[derive(SystemSet, Hash, Debug, PartialEq, Eq, Clone)]
pub struct TerrainSet;

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin)
            .register_type::<TerrainSettings>()
            .register_type::<CursorWorldPos>()
            .register_type::<HoveredTile>()
            .init_resource::<TerrainSettings>()
            .insert_resource(CursorWorldPos(Vec3::new(-100., -100., 0.)))
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

#[derive(Component, Default)]
pub struct Terrain {
    pub terrain_texture: Handle<Image>,
    pub chunks: HashMap<IVec2, Entity>,
}

impl Terrain {
    pub fn new(terrain_texture: Handle<Image>) -> Self {
        Self {
            terrain_texture,
            ..default()
        }
    }
}

#[derive(Resource, Debug, Reflect)]
pub struct TerrainSettings {
    seed: u32,
    chunk_spawn_radius: i32,
    #[cfg(feature = "async")]
    enable_async: bool,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            seed: 1234567,
            chunk_spawn_radius: 5,
            #[cfg(feature = "async")]
            enable_async: true,
        }
    }
}

#[derive(Component, Debug, Reflect, Default)]
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
pub struct TerrainBundle {
    pub terrain: Terrain,
    pub generator: TerrainGenerator,
    pub name: Name,
    pub transform: TransformBundle,
    pub visibility: VisibilityBundle,
}

impl Default for TerrainBundle {
    fn default() -> Self {
        Self {
            terrain: default(),
            generator: TerrainGenerator::new(Box::new(FlatChunkGenerator::new(0))),
            name: Name::new("Terrain"),
            transform: default(),
            visibility: default(),
        }
    }
}

#[derive(Bundle, Default)]
pub struct ChunkBundle {
    chunk: Chunk,
    transform: TransformBundle,
    visibility: VisibilityBundle,
}

fn queue_chunk_generation(
    commands: &mut Commands,
    terrain_entity: Entity,
    terrain: &mut Terrain,
    chunk_position: IVec2,
    generator: &TerrainGenerator,
    #[cfg(feature = "async")] enable_async: bool,
) {
    let IVec2 { x, y } = chunk_position;

    debug!("Queueing chunk generation at {:?}", chunk_position);
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
            let chunk_entity = parent
                .spawn((
                    GenerateChunk(task),
                    Chunk {
                        position: chunk_position,
                    },
                    TransformBundle::from_transform(transform),
                    VisibilityBundle::default(),
                ))
                .id();

            terrain.chunks.insert(chunk_position, chunk_entity);
        });
        return;
    }

    #[cfg(not(feature = "async"))]
    {
        let chunk_data = generator.generate_chunk(chunk_position);
        commands.entity(terrain_entity).with_children(|parent| {
            let chunk_entity = parent
                .spawn((
                    chunk_data,
                    ChunkBundle {
                        chunk: Chunk {
                            position: chunk_position,
                        },
                        transform: TransformBundle::from_transform(transform),
                        ..default()
                    },
                ))
                .id();

            terrain.chunks.insert(chunk_position, chunk_entity);
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

#[derive(Component, Debug, Reflect)]
struct SpawnedChunkTilemap;

fn spawn_generated_chunks(
    mut commands: Commands,
    mut chunk_task: Query<
        (Entity, &ChunkData, &Parent),
        (Added<ChunkData>, Without<SpawnedChunkTilemap>),
    >,
    terrain_query: Query<&Terrain>,
) {
    for (chunk_entity, chunk_data, parent) in &mut chunk_task {
        let terrain = terrain_query
            .get(parent.get())
            .expect("Terrain entity not found");
        add_chunk_tilemap(
            &mut commands,
            chunk_data,
            chunk_entity,
            terrain.terrain_texture.clone(),
        );
    }
}

fn add_chunk_tilemap(
    commands: &mut Commands,
    chunk_data: &ChunkData,
    chunk_entity: Entity,
    texture_handle: Handle<Image>,
) {
    let map_type = TilemapType::Square;
    let tilemap_entity = {
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
        tilemap_entity
    };

    commands
        .entity(chunk_entity)
        .add_child(tilemap_entity)
        .insert(SpawnedChunkTilemap);
}

fn spawn_chunks_around_camera(
    camera_query: Query<&GlobalTransform, With<Camera>>,
    terrain_settings: Res<TerrainSettings>,
    mut terrain_params: TerrainParams,
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
                let (terrain_entity, terrain) = terrain_params.terrain_query.single();
                if terrain.chunks.contains_key(&chunk_position) {
                    continue;
                }
                terrain_params.queue_spawn_chunk(terrain_entity, chunk_position);
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
    terrain_query: Query<'w, 's, (Entity, &'static mut Terrain)>,
    transform_query: Query<'w, 's, &'static GlobalTransform>,
    generator_query: Query<'w, 's, &'static TerrainGenerator>,
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
}

impl<'w, 's> TerrainParams<'w, 's> {
    pub fn queue_spawn_chunk(&mut self, terrain_entity: Entity, chunk_position: IVec2) {
        let generator = self
            .generator_query
            .get(terrain_entity)
            .expect("Terrain entity not found");

        let mut terrain = self
            .terrain_query
            .get_mut(terrain_entity)
            .expect("Terrain entity not found")
            .1;

        if terrain.chunks.contains_key(&chunk_position) {
            debug!("Chunk at {:?} already spawned", chunk_position);
            return;
        }

        queue_chunk_generation(
            &mut self.commands,
            terrain_entity,
            &mut terrain,
            chunk_position,
            generator,
            #[cfg(feature = "async")]
            self.terrain_settings.enable_async,
        );
    }

    /// Retrieve the tile entity at a given world position.
    ///
    /// # Arguments
    /// * `world_position` - The position in the world.
    ///
    /// # Returns
    /// An `Option<Entity>` representing the tile entity at the specified world position.
    /// Returns `None` if no tile entity is found at that position.
    pub fn tile_entity_at_global_pos(&self, world_position: Vec2) -> Option<Entity> {
        // Retrieve the global transform of the terrain to convert world positions
        let (terrain_entity, terrain) = self.terrain_query.single();
        let terrain_transform = self
            .transform_query
            .get(terrain_entity)
            .expect("Terrain transform not found");

        // Transform the world position to the terrain's local coordinate system
        let terrain_local_position = terrain_transform
            .compute_matrix()
            .inverse()
            .transform_point3(world_position.extend(0.0))
            .truncate();

        // Calculate the coordinates of the chunk based on the local position within the terrain
        let chunk_coordinates = terrain_pos_to_chunk_id(terrain_local_position);

        // Retrieve the tilemap entity and tile storage for the chunk that contains the given position
        terrain
            .chunks
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
    use bevy::{ecs::system::SystemState, utils::HashSet};

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

    #[test]
    fn test_terrain_plugin() {
        let mut app = App::new();
        app.add_plugins((AssetPlugin::default(), TerrainPlugin));

        app.world.spawn(TerrainBundle::default());
        *app.world.get_resource_mut::<State<AppState>>().unwrap() = State::new(AppState::Running);

        app.update();

        // Check if the terrain entity is spawned
        {
            let mut system_state: SystemState<Query<&Terrain>> = SystemState::new(&mut app.world);
            let terrain_spawned = !system_state.get_mut(&mut app.world).is_empty();
            assert!(terrain_spawned, "Terrain entity not found");
        }
    }

    #[test]
    fn test_queue_spawn_chunk() {
        let mut app = App::new();

        // Setup terrain
        let terrain_entity = app.world.spawn(TerrainBundle::default()).id();

        {
            // Get system state to trigger spawning chunk (0, 0)
            let mut system_state: SystemState<TerrainParams> = SystemState::new(&mut app.world);
            let mut terrain_params = system_state.get_mut(&mut app.world);
            terrain_params.queue_spawn_chunk(terrain_entity, IVec2::new(0, 0));
        }

        app.update();

        {
            let mut system_state: SystemState<(
                Commands,
                Query<(Entity, &ChunkData)>,
                Query<&Terrain>,
            )> = SystemState::new(&mut app.world);
            let (mut commands, chunk_task, terrain_query) = system_state.get_mut(&mut app.world);
            let (chunk_entity, chunk_data) = chunk_task.single();
            let terrain = terrain_query.single();
            add_chunk_tilemap(
                &mut commands,
                chunk_data,
                chunk_entity,
                terrain.terrain_texture.clone(),
            );
        }

        // Check if the tiles are spawned
        {
            let mut system_state: SystemState<(Query<(Entity, &TilePos)>, TerrainParams)> =
                SystemState::new(&mut app.world);
            let (tile_query, terrain_params) = system_state.get_mut(&mut app.world);

            let tile_positions: HashSet<TilePos> =
                tile_query.iter().map(|(_, tile_pos)| *tile_pos).collect();
            let expected_tile_positions: HashSet<TilePos> = (0..CHUNK_SIZE.x)
                .flat_map(|x| (0..CHUNK_SIZE.y).map(move |y| TilePos::new(x, y)))
                .collect();

            assert_eq!(
                tile_positions, expected_tile_positions,
                "Tile positions are not correct"
            );
        }
    }
}
