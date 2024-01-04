mod debug;
pub mod terrain_generator;

#[cfg(feature = "async")]
use bevy::tasks::{AsyncComputeTaskPool, Task};
#[cfg(feature = "async")]
use futures_lite::future;

use bevy::{
    ecs::system::{RunSystemOnce, SystemParam, SystemState},
    math::Vec3Swizzles,
    prelude::*,
    utils::HashMap,
};

use bevy_ecs_tilemap::prelude::*;
use ndarray::prelude::*;

use kloonorio_core::{item::Item, mineable::Mineable, player::Player, types::AppState};

use self::{
    debug::{chunk_gizmos, hovered_tile_gizmo},
    terrain_generator::{FlatChunkGenerator, TerrainGenerator},
};

pub use self::debug::TerrainDebug;

#[derive(SystemSet, Hash, Debug, PartialEq, Eq, Clone)]
pub struct TerrainSet;

pub struct KloonorioTerrainPlugin;

impl Plugin for KloonorioTerrainPlugin {
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
pub struct ChunkData {
    tiles: Array2<Option<u32>>,
    ores: HashMap<UVec2, u32>,
}

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

fn spawn_empty_chunk(
    commands: &mut Commands,
    terrain: &mut Terrain,
    terrain_entity: Entity,
    chunk_position: IVec2,
) -> Entity {
    let transform = Transform::from_translation(Vec3::new(
        chunk_position.x as f32 * CHUNK_SIZE.x as f32,
        chunk_position.y as f32 * CHUNK_SIZE.y as f32,
        0.0,
    ));

    let chunk_entity = commands
        .spawn((
            Chunk {
                position: chunk_position,
            },
            TransformBundle::from_transform(transform),
            VisibilityBundle::default(),
        ))
        .id();

    terrain.chunks.insert(chunk_position, chunk_entity);
    commands
        .entity(terrain_entity)
        .push_children(&[chunk_entity]);

    chunk_entity
}

fn spawn_and_populate_new_chunk(
    commands: &mut Commands,
    terrain_entity: Entity,
    terrain: &mut Terrain,
    chunk_position: IVec2,
    generator: &TerrainGenerator,
    #[cfg(feature = "async")] enable_async: bool,
) -> Entity {
    debug!("Queueing chunk generation at {:?}", chunk_position);

    let chunk_entity = spawn_empty_chunk(commands, terrain, terrain_entity, chunk_position);

    #[cfg(feature = "async")]
    if enable_async {
        let thread_pool = AsyncComputeTaskPool::get();
        let generator_clone = generator.clone();
        let task = thread_pool.spawn(async move { generator_clone.generate_chunk(chunk_position) });

        commands.entity(chunk_entity).insert(GenerateChunk(task));

        return;
    }

    let chunk_data = generator.generate_chunk(chunk_position);
    commands.entity(chunk_entity).insert(chunk_data);

    chunk_entity
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
        let tiles_data = chunk_data.tiles.view();
        let tilemap_size = TilemapSize {
            x: tiles_data.ncols() as u32,
            y: tiles_data.nrows() as u32,
        };

        let tilemap_entity = commands.spawn_empty().id();
        let mut tile_storage = TileStorage::empty(tilemap_size);
        for ((x, y), tile) in tiles_data.indexed_iter() {
            let x = x as u32;
            let y = y as u32;
            if let Some(texture_id) = tile {
                let tile_pos = TilePos { x, y };

                let mut tile_entity_commands = commands.spawn(TileBundle {
                    position: tile_pos,
                    tilemap_id: TilemapId(tilemap_entity),
                    texture_index: TileTextureIndex(*texture_id),

                    ..default()
                });
                if chunk_data.ores.contains_key(&UVec2::new(x, y)) {
                    let product = match *texture_id {
                        COAL => Item::new("Coal"),
                        IRON => Item::new("Iron ore"),
                        STONE => Item::new("Stone"),
                        _ => panic!("Invalid ore type"),
                    };

                    tile_entity_commands.insert(Mineable(product));
                }
                let tile_entity = tile_entity_commands.id();
                commands.entity(tilemap_entity).add_child(tile_entity);
                tile_storage.set(&tile_pos, tile_entity);
            } else {
                warn!("Tile at {:?} is empty", TilePos { x, y });
            }
        }

        let map_transform = Transform::from_translation(Vec3::new(
            -(0.5 * CHUNK_SIZE.x as f32) + 0.5,
            -(0.5 * CHUNK_SIZE.y as f32) + 0.5,
            0.0,
        ));

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
                terrain_params.spawn_chunk(terrain_entity, chunk_position);
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
    pub fn spawn_chunk(&mut self, terrain_entity: Entity, chunk_position: IVec2) -> Option<Entity> {
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
            return None;
        }

        Some(spawn_and_populate_new_chunk(
            &mut self.commands,
            terrain_entity,
            &mut terrain,
            chunk_position,
            generator,
            #[cfg(feature = "async")]
            self.terrain_settings.enable_async,
        ))
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
                    .transform_point(world_position.extend(0.))
                    .xy();

                // Calculate the tile position from the local position within the tilemap
                let tile_pos = TilePos::from_world_pos(
                    &local_tilemap_position,
                    &CHUNK_SIZE.into(),
                    tilemap_grid_size,
                    &TilemapType::Square,
                );

                // Retrieve the tile entity from the tile storage
                let tile_entity = tile_pos.and_then(|tile_pos| tile_storage.get(&tile_pos));
                trace!("Tile at {:?} is {:?}", local_tilemap_position, tile_pos);
                tile_entity
            })
    }

    pub fn tile_texture_index(&self, tile_entity: Entity) -> Option<TileTextureIndex> {
        self.tiles.get(tile_entity).ok().copied()
    }
}

pub fn spawn_test_terrain(app: &mut App) -> Option<Entity> {
    // Setup terrain
    let terrain_entity = app.world.spawn(TerrainBundle::default()).id();
    let chunk_entity = {
        let mut system_state = SystemState::<TerrainParams>::new(&mut app.world);
        let mut terrain_params = system_state.get_mut(&mut app.world);
        let chunk_entity = terrain_params.spawn_chunk(terrain_entity, IVec2::ZERO);
        system_state.apply(&mut app.world);
        chunk_entity
    };
    app.world.run_system_once(spawn_generated_chunks);
    app.world.run_system_once(apply_deferred);
    // app.add_systems(Update, spawn_generated_chunks);
    // app.update();
    chunk_entity
}

#[cfg(test)]
pub mod test {
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
    fn test_terrain_params_spawn_chunk() {
        let mut app = App::new();
        app.add_plugins(TransformPlugin);

        // Setup terrain
        spawn_test_terrain(&mut app).expect("Terrain entity not found");

        // Check tiles and their positions
        let mut system_state = SystemState::<(TerrainParams, Query<&TilePos>)>::new(&mut app.world);
        let (terrain_params, tile_pos_query) = system_state.get_mut(&mut app.world);

        let mut tile_positions = HashSet::new();
        for tile_pos in tile_pos_query.iter() {
            tile_positions.insert(*tile_pos);
        }

        for x in 0..CHUNK_SIZE.x {
            for y in 0..CHUNK_SIZE.y {
                assert!(
                    tile_positions.contains(&TilePos::new(x, y)),
                    "Tile at {:?} not spawned",
                    TilePos::new(x, y)
                );
            }
        }

        // Check specific tiles and their positions
        let expected_tile_positions = vec![
            (Vec2::new(-4.0, -4.0), TilePos::new(0, 0)),
            (Vec2::new(0.0, 0.0), TilePos::new(4, 4)),
            (Vec2::new(4., -4.), TilePos::new(8, 0)),
        ];

        for (global_pos, expected_tile_pos) in expected_tile_positions {
            let tile_entity = terrain_params
                .tile_entity_at_global_pos(global_pos)
                .unwrap_or_else(|| panic!("Tile at {:?} not found", global_pos));
            let tile_pos = tile_pos_query.get(tile_entity).expect("TilePos not found");
            assert_eq!(
                *tile_pos, expected_tile_pos,
                "Tile at {:?} has incorrect position",
                global_pos
            );
        }
    }
}
