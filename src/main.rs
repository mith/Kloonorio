use bevy::{
    diagnostic::FrameTimeDiagnosticsPlugin, input::mouse::MouseWheel, math::Vec3Swizzles,
    prelude::*,
};
use bevy_ecs_tilemap::tiles::TileTextureIndex;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_rapier2d::prelude::*;
use egui::Align2;

use inserter::inserter_tick;
use isometric_sprite::IsometricSpritePlugin;
use iyes_loopless::prelude::*;

mod building_ui;
mod burner;
mod character_ui;
mod drag_and_drop;
mod inserter;
mod intermediate_loader;
mod inventory;
mod inventory_grid;
mod isometric_sprite;
mod loading;
mod miner;
mod placeable;
mod player_movement;
mod recipe_loader;
mod smelter;
mod structure_loader;
mod terrain;
mod types;
mod util;

use crate::{
    building_ui::building_ui,
    burner::{burner_load, burner_tick},
    character_ui::CharacterUiPlugin,
    drag_and_drop::drop_system,
    intermediate_loader::IntermediateLoaderPlugin,
    inventory::Inventory,
    inventory_grid::{Hand, SlotEvent},
    loading::LoadingPlugin,
    miner::miner_tick,
    placeable::Building,
    player_movement::PlayerMovementPlugin,
    recipe_loader::{RecipeAsset, RecipeLoaderPlugin},
    smelter::smelter_tick,
    structure_loader::StructureLoaderPlugin,
    terrain::{CursorPos, HoveredTile, TerrainPlugin, COAL, IRON, STONE, TREE},
    types::{AppState, CraftingQueue, GameState, Player, Powered, Product, UiPhase, Working},
};

fn main() {
    let mut app = App::new();
    app.init_resource::<GameState>()
        .insert_resource(PlayerSettings {
            max_mining_distance: 20.,
        })
        .insert_resource(CameraSettings {
            min_zoom: 0.1,
            max_zoom: 10.,
        })
        .add_loopless_state(AppState::Loading)
        // .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    watch_for_changes: true,
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugin(EguiPlugin)
        // .insert_resource(WorldInspectorParams {
        //     name_filter: Some("Interesting".into()),
        //     ..default()
        // })
        // .add_plugin(WorldInspectorPlugin::new())
        .insert_resource(RapierConfiguration {
            gravity: Vec2::new(0.0, 0.0),
            ..default()
        })
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(TerrainPlugin)
        .add_plugin(IsometricSpritePlugin)
        .add_plugin(CharacterUiPlugin)
        .add_plugin(PlayerMovementPlugin)
        .add_plugin(RecipeLoaderPlugin)
        .add_plugin(StructureLoaderPlugin)
        .add_plugin(IntermediateLoaderPlugin)
        .add_plugin(LoadingPlugin)
        .add_enter_system(AppState::Running, spawn_player)
        .add_event::<SlotEvent>()
        .add_system_set(
            ConditionSet::new()
                .run_in_state(AppState::Running)
                .with_system(camera_zoom)
                .with_system(interact)
                .with_system(interact_completion)
                .with_system(interact_cancel)
                .with_system(interaction_ui)
                .with_system(craft_ticker)
                .with_system(pick_building)
                .with_system(building_ui)
                .with_system(smelter_tick)
                .with_system(burner_tick)
                .with_system(burner_load)
                // .with_system(working_texture)
                .with_system(miner_tick)
                .with_system(inserter_tick)
                .with_system(hovering_ui)
                .with_system(placeable::placeable)
                .with_system(placeable::placeable_rotation)
                .into(),
        )
        .add_system(drop_system.run_in_state(AppState::Running).after(UiPhase))
        .run();
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SelectedBuilding(Entity);

fn pick_building(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mouse_input: Res<Input<MouseButton>>,
    building_query: Query<&Building>,
    player_query: Query<Entity, With<Player>>,
    cursor_pos: Res<CursorPos>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let cursor: Vec2 = cursor_pos.0.xy();
    rapier_context.intersections_with_point(cursor, QueryFilter::new(), |entity| {
        if let Ok(_building) = building_query.get(entity) {
            let player = player_query.single();
            commands.entity(player).insert(SelectedBuilding(entity));
            info!("Selected building: {:?}", entity);
            return false;
        }
        true
    });
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HoveringUI;

fn hovering_ui(
    mut commands: Commands,
    mut egui_context: ResMut<EguiContext>,
    hovering_player_query: Query<Entity, (With<Player>, With<HoveringUI>)>,
    non_hovering_player_query: Query<Entity, (With<Player>, Without<HoveringUI>)>,
) {
    if egui_context.ctx_mut().is_pointer_over_area() {
        for entity in non_hovering_player_query.iter() {
            commands.entity(entity).insert(HoveringUI);
        }
    } else {
        for entity in hovering_player_query.iter() {
            commands.entity(entity).remove::<HoveringUI>();
        }
    }
}

fn working_texture(
    mut buildings: ParamSet<(
        Query<&mut TextureAtlasSprite, (With<Powered>, With<Working>)>,
        Query<&mut TextureAtlasSprite, Without<Powered>>,
        Query<&mut TextureAtlasSprite, Without<Working>>,
    )>,
) {
    for mut active_sprite in buildings.p0().iter_mut() {
        active_sprite.index = 1;
    }

    for mut unpowered_sprite in buildings.p1().iter_mut() {
        unpowered_sprite.index = 0;
    }

    for mut idle_sprite in buildings.p2().iter_mut() {
        idle_sprite.index = 0;
    }
}

fn spawn_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut inventory = Inventory::new(100);
    inventory.add_item(Product::Structure("Wooden chest".into()), 100);
    inventory.add_item(Product::Structure("Burner mining drill".into()), 100);
    inventory.add_item(Product::Structure("Stone furnace".into()), 100);
    inventory.add_item(Product::Structure("Burner inserter".into()), 100);
    inventory.add_item(Product::Intermediate("Coal".into()), 200);
    inventory.add_item(Product::Intermediate("Iron ore".into()), 200);
    commands
        .spawn((
            SpriteBundle {
                texture: asset_server.load("textures/character.png"),
                transform: Transform::from_xyz(0.0, 0.0, 1.0),
                ..Default::default()
            },
            Player,
            Hand::default(),
            inventory,
            CraftingQueue::default(),
        ))
        .with_children(|parent| {
            parent.spawn(Camera2dBundle {
                transform: Transform::from_xyz(0.0, 0.0, 500.0),
                projection: OrthographicProjection {
                    scale: 0.3,
                    ..Default::default()
                },
                ..default()
            });
        });
}

#[derive(Resource)]
struct CameraSettings {
    min_zoom: f32,
    max_zoom: f32,
}

fn camera_zoom(
    mut query: Query<&mut OrthographicProjection>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    camera_settings: Res<CameraSettings>,
) {
    for mut projection in &mut query {
        for event in mouse_wheel_events.iter() {
            projection.scale -= event.y * 0.1;
            projection.scale = projection
                .scale
                .max(camera_settings.min_zoom)
                .min(camera_settings.max_zoom);
        }
    }
}

#[derive(Component)]
struct MineCountdown {
    timer: Timer,
    target: Entity,
}

fn is_minable(tile: u32) -> bool {
    matches!(tile, COAL | IRON | STONE | TREE)
}

#[derive(Resource)]
struct PlayerSettings {
    max_mining_distance: f32,
}

fn interact(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tile_query: Query<&TileTextureIndex>,
    mouse_button_input: Res<Input<MouseButton>>,
    player_query: Query<(Entity, &Transform, &HoveredTile), (With<Player>, Without<MineCountdown>)>,
    player_settings: Res<PlayerSettings>,
) {
    if player_query.is_empty() {
        return;
    }
    let (player_entity, player_transform, hovered_tile) = player_query.single();

    if !mouse_button_input.pressed(MouseButton::Right) {
        return;
    };

    if let Ok(tile_texture) = tile_query.get(hovered_tile.entity) {
        let tile_distance = player_transform
            .translation
            .xy()
            .distance(cursor_pos.0.xy());
        if is_minable(tile_texture.0) && tile_distance < player_settings.max_mining_distance {
            commands.entity(player_entity).insert(MineCountdown {
                timer: Timer::from_seconds(1.0, TimerMode::Repeating),
                target: hovered_tile.entity,
            });
        }
    }
}

fn interact_cancel(
    mut commands: Commands,
    player_query: Query<Entity, With<MineCountdown>>,
    mouse_button_input: Res<Input<MouseButton>>,
) {
    if mouse_button_input.just_released(MouseButton::Right) {
        for player_entity in &player_query {
            commands.entity(player_entity).remove::<MineCountdown>();
        }
    }
}

fn interact_completion(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Inventory, &mut MineCountdown)>,
    tile_query: Query<&TileTextureIndex>,
) {
    for (entity, mut inventory, mut interaction) in &mut query {
        if interaction.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).remove::<MineCountdown>();
            let tile_entity = interaction.target;
            if let Ok(tile_texture) = tile_query.get(tile_entity) {
                match tile_texture.0 {
                    COAL => inventory.add_item(Product::Intermediate("Coal".into()), 1),
                    IRON => inventory.add_item(Product::Intermediate("Iron ore".into()), 1),
                    STONE => inventory.add_item(Product::Intermediate("Stone".into()), 1),
                    TREE => inventory.add_item(Product::Intermediate("Wood".into()), 1),
                    _ => vec![],
                };
            }
        }
    }
}

fn interaction_ui(mut egui_context: ResMut<EguiContext>, interact_query: Query<&MineCountdown>) {
    if let Ok(interact) = interact_query.get_single() {
        egui::Window::new("Interaction")
            .anchor(Align2::CENTER_BOTTOM, (0., -10.))
            .title_bar(false)
            .resizable(false)
            .show(egui_context.ctx_mut(), |ui| {
                ui.add(egui::ProgressBar::new(interact.timer.percent()));
            });
    }
}

fn craft_ticker(
    mut player_query: Query<(&mut Inventory, &mut CraftingQueue), With<Player>>,
    time: Res<Time>,
) {
    for (mut inventory, mut build_queue) in &mut player_query {
        if let Some(active_build) = build_queue.0.front_mut() {
            if active_build.timer.tick(time.delta()).just_finished() {
                inventory.add_items(&active_build.blueprint.products);
                build_queue.0.pop_front();
            }
        }
    }
}
