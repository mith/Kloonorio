use bevy::{
    asset::AssetServerSettings,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseWheel,
    math::Vec3Swizzles,
    prelude::*,
    render::texture::ImageSettings,
};
use bevy_ecs_tilemap::tiles::TileTexture;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_rapier2d::prelude::*;
use egui::Align2;

mod inventory;
mod player_movement;
mod terrain;
mod types;

use crate::inventory::{ActiveCraft, CraftingQueue, Inventory, InventoryPlugin, Stack};
use crate::player_movement::PlayerMovementPlugin;
use crate::terrain::{CursorPos, HoveredTile, TerrainPlugin, COAL, IRON, STONE, TREE};
use crate::types::{AppState, CursorState, GameState, Player};

use crate::inventory::{Building, Fueled, Smelter};
use crate::types::Resource;

fn main() {
    let mut app = App::new();
    app.insert_resource(AssetServerSettings {
        watch_for_changes: true,
        ..default()
    })
    .insert_resource(ImageSettings::default_nearest())
    .init_resource::<GameState>()
    .init_resource::<CursorState>()
    .insert_resource(PlayerSettings {
        max_mining_distance: 20.,
    })
    .insert_resource(CameraSettings {
        min_zoom: 0.1,
        max_zoom: 10.,
    })
    .insert_resource(vec![Recipe {
        materials: vec![(Resource::Stone, 5u32)],
        products: vec![(Resource::StoneFurnace, 1u32)],
        crafting_time: 0.5,
        texture: "stone_furnace.png".to_string(),
        name: "Stone Furnace".to_string(),
    }])
    // .add_plugin(LogDiagnosticsPlugin::default())
    .add_plugin(FrameTimeDiagnosticsPlugin::default())
    .add_plugins(DefaultPlugins)
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
    // .add_plugin(RapierDebugRenderPlugin::default())
    .add_plugin(TerrainPlugin)
    .add_plugin(InventoryPlugin)
    .add_plugin(PlayerMovementPlugin)
    .add_state(AppState::Setup)
    .add_startup_system(setup)
    .add_system(camera_zoom)
    // .add_system(performance_ui)
    .add_system(interact)
    .add_system(interact_completion)
    .add_system(interact_cancel)
    .add_system(interaction_ui)
    .add_system(craft_ui)
    .add_system(craft_ticker)
    .add_system(pick_building)
    .add_system(building_ui)
    .add_system(smelter_tick)
    .add_system(fueled_tick)
    .add_system(fueled_load)
    .add_system(working_texture)
    .run();
}

#[derive(Component)]
struct SelectedBuilding(Entity);

fn pick_building(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mouse_input: Res<Input<MouseButton>>,
    building_query: Query<&Building>,
    player_query: Query<Entity, With<Player>>,
    cursor_pos: Res<CursorPos>,
) {
    if mouse_input.just_pressed(MouseButton::Left) {
        let cursor: Vec2 = cursor_pos.0.xy();
        rapier_context.intersections_with_point(cursor, QueryFilter::new(), |entity| {
            if let Ok(_building) = building_query.get(entity) {
                let player = player_query.single();
                commands
                    .entity(player)
                    .insert(SelectedBuilding(entity))
                    .insert(Name::new("Interesting"));
                return false;
            }
            true
        });
    }
}

#[derive(Component)]
struct Powered;

#[derive(Component)]
struct Working;

fn smelter_tick(
    mut commands: Commands,
    mut smelter_query: Query<
        (Entity, &mut Smelter, &mut CraftingQueue, &mut Inventory),
        With<Powered>,
    >,
    time: Res<Time>,
) {
    for (entity, mut smelter, mut crafting_queue, mut inventory) in smelter_query.iter_mut() {
        if inventory.has_items(&[(Resource::Iron, 1)])
            && crafting_queue.0.is_empty()
            && smelter.output.can_add(&[(Resource::IronPlate, 1)])
        {
            inventory.remove_items(&[(Resource::Iron, 1)]);
            crafting_queue.0.push_back(ActiveCraft {
                timer: Timer::from_seconds(1., false),
                blueprint: Recipe {
                    materials: vec![(Resource::Iron, 1u32)],
                    products: vec![(Resource::IronPlate, 1u32)],
                    crafting_time: 0.5,
                    texture: "iron_plate.png".to_string(),
                    name: "Iron Plate".to_string(),
                },
            });
            commands.entity(entity).insert(Working);
        }

        if let Some(active_build) = crafting_queue.0.front_mut() {
            if active_build.timer.tick(time.delta()).just_finished() {
                smelter.output.add_items(&active_build.blueprint.products);
                crafting_queue.0.pop_front();
                commands.entity(entity).remove::<Working>();
            }
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

fn fueled_tick(
    mut commands: Commands,
    mut fueled_query: Query<(Entity, &mut Fueled), With<Working>>,
    time: Res<Time>,
) {
    for (entity, mut fueled) in fueled_query.iter_mut() {
        if let Some(timer) = &mut fueled.fuel_timer {
            if timer.tick(time.delta()).just_finished() {
                commands.entity(entity).remove::<Powered>();
                fueled.fuel_timer = None;
            }
        }
    }
}

fn fueled_load(
    mut commands: Commands,
    mut fueled_query: Query<(Entity, &mut Fueled), Without<Powered>>,
) {
    for (entity, mut fueled) in fueled_query.iter_mut() {
        if fueled.fuel_inventory.remove_items(&[(Resource::Coal, 1)]) {
            fueled.fuel_timer = Some(Timer::from_seconds(10., false));
            commands.entity(entity).insert(Powered);
        }
    }
}

fn building_ui(
    mut commands: Commands,
    mut egui_ctx: ResMut<EguiContext>,
    mut player_query: Query<
        (Entity, &SelectedBuilding, &mut Inventory),
        (With<Player>, Without<Building>),
    >,
    name: Query<&Name>,
    mut smelter_query: Query<(&mut Smelter, &mut Inventory), With<Building>>,
    mut fueled_query: Query<&mut Fueled, With<Building>>,
    crafting_queue_query: Query<&CraftingQueue, With<Building>>,
) {
    for (player_entity, SelectedBuilding(selected_building), mut player_inventory) in
        &mut player_query
    {
        let name = name
            .get(*selected_building)
            .map_or("Building", |n| n.as_str());

        let mut window_open = true;
        egui::Window::new(name)
            .open(&mut window_open)
            .show(egui_ctx.ctx_mut(), |ui| {
                if let Ok((mut smelter, mut building_inventory)) =
                    smelter_query.get_mut(*selected_building)
                {
                    ui.horizontal(|ui| {
                        ui.label("Input:");
                        ui.label(format!("{:?}", building_inventory.slots[0]));
                        if player_inventory.has_items(&[(Resource::Iron, 1)])
                            && ui.button("Load").clicked()
                        {
                            player_inventory.remove_items(&[(Resource::Iron, 1)]);
                            let remainder = building_inventory.add_items(&[(Resource::Iron, 1)]);
                            player_inventory.add_items(&remainder);
                        }
                    });
                    if let Ok(crafting_queue) = crafting_queue_query.get(*selected_building) {
                        if let Some(active_craft) = crafting_queue.0.front() {
                            ui.add(egui::ProgressBar::new(active_craft.timer.percent()));
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("Output:");
                        ui.label(format!("{:?}", smelter.output.slots[0]));
                        if !smelter.output.empty() && ui.button("Unload").clicked() {
                            player_inventory.add_items(&smelter.output.take_all());
                        }
                    });
                }

                if let Ok(mut fueled) = fueled_query.get_mut(*selected_building) {
                    ui.horizontal(|ui| {
                        ui.label("Fuel:");
                        ui.label(format!("{:?}", fueled.fuel_inventory.slots[0]));

                        if player_inventory.has_items(&[(Resource::Coal, 1)])
                            && ui.button("Load").clicked()
                        {
                            player_inventory.remove_items(&[(Resource::Coal, 1)]);
                            fueled.fuel_inventory.add_item(Resource::Coal, 1);
                        }
                    });
                    if let Some(timer) = &fueled.fuel_timer {
                        ui.add(egui::ProgressBar::new(1. - timer.percent()));
                    }
                }
            });
        if !window_open {
            commands.entity(player_entity).remove::<SelectedBuilding>();
        }
    }
}

#[derive(Clone)]
pub struct Recipe {
    pub materials: Vec<(Resource, u32)>,
    pub products: Vec<(Resource, u32)>,
    pub crafting_time: f32,
    pub texture: String,
    pub name: String,
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    asset_server.watch_for_changes().unwrap();
    commands
        .spawn_bundle(SpriteBundle {
            texture: asset_server.load("textures/character.png"),
            transform: Transform::from_xyz(0.0, 0.0, 1.0),
            ..Default::default()
        })
        .insert(Player)
        .insert(Inventory {
            slots: [
                vec![None; 10],
                vec![
                    Some(Stack {
                        resource: Resource::StoneFurnace,
                        amount: 1,
                    }),
                    Some(Stack {
                        resource: Resource::Coal,
                        amount: 1,
                    }),
                    Some(Stack {
                        resource: Resource::Iron,
                        amount: 1,
                    }),
                ],
            ]
            .concat(),
        })
        .insert(CraftingQueue::default())
        .with_children(|parent| {
            parent.spawn_bundle(Camera2dBundle {
                transform: Transform::from_xyz(0.0, 0.0, 500.0),
                projection: OrthographicProjection {
                    scale: 0.3,
                    ..Default::default()
                },
                ..default()
            });
        });
}

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

fn performance_ui(mut egui_context: ResMut<EguiContext>, diagnostics: Res<Diagnostics>) {
    egui::Window::new("Performance").show(egui_context.ctx_mut(), |ui| {
        if let Some(diagnostic) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(average) = diagnostic.average() {
                ui.label(format!("FPS: {:.2}", average));
            }
        }
    });
}

#[derive(Component)]
struct MineCountdown {
    timer: Timer,
    target: Entity,
}

fn is_minable(tile: u32) -> bool {
    matches!(tile, COAL | IRON | STONE | TREE)
}

struct PlayerSettings {
    max_mining_distance: f32,
}

fn interact(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    tile_query: Query<&TileTexture>,
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
        info!("Tile distance: {}", tile_distance);
        if is_minable(tile_texture.0) && tile_distance < player_settings.max_mining_distance {
            commands.entity(player_entity).insert(MineCountdown {
                timer: Timer::from_seconds(1.0, false),
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
    tile_query: Query<&TileTexture>,
) {
    for (entity, mut inventory, mut interaction) in &mut query {
        if interaction.timer.tick(time.delta()).just_finished() {
            info!("Interact complete");
            commands.entity(entity).remove::<MineCountdown>();
            let tile_entity = interaction.target;
            if let Ok(tile_texture) = tile_query.get(tile_entity) {
                match tile_texture.0 {
                    COAL => inventory.add_item(Resource::Coal, 1),
                    IRON => inventory.add_item(Resource::Iron, 1),
                    STONE => inventory.add_item(Resource::Stone, 1),
                    TREE => inventory.add_item(Resource::Wood, 1),
                    _ => {}
                }
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

fn craft_ui(
    mut egui_context: ResMut<EguiContext>,
    blueprints: Res<Vec<Recipe>>,
    mut player_query: Query<(&mut Inventory, &mut CraftingQueue), With<Player>>,
) {
    egui::Window::new("Crafting").show(egui_context.ctx_mut(), |ui| {
        for blueprint in blueprints.iter() {
            ui.horizontal(|ui| {
                ui.label(&blueprint.name);
                ui.label(format!("Time: {:.2}", blueprint.crafting_time));
                ui.label("Materials:".to_string());
                for (resource, amount) in &blueprint.materials {
                    ui.label(format!("{:?}: {}", resource, amount));
                }
                for (mut inventory, mut build_queue) in &mut player_query {
                    let resources_available = inventory.has_items(&blueprint.materials);
                    if ui
                        .add_enabled(resources_available, egui::Button::new("Craft"))
                        .clicked()
                    {
                        inventory.remove_items(&blueprint.materials);
                        build_queue.0.push_back(ActiveCraft {
                            blueprint: blueprint.clone(),
                            timer: Timer::from_seconds(blueprint.crafting_time, false),
                        });
                    }
                }
            });
        }

        for (mut inventory, mut build_queue) in &mut player_query {
            ui.separator();
            ui.label("Crafting queue");
            ui.separator();
            let mut to_remove = Vec::new();
            for (i, active_build) in build_queue.0.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(&active_build.blueprint.name);
                    ui.label(format!(
                        "Time remaining: {:.2}",
                        active_build.timer.percent()
                    ));
                    if ui.button("Cancel").clicked() {
                        inventory.add_items(&active_build.blueprint.materials);
                        to_remove.push(i);
                    }
                });
            }
            for i in to_remove {
                build_queue.0.remove(i);
            }
        }
    });
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
