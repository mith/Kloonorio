use bevy::{
    asset::AssetServerSettings,
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    input::mouse::MouseWheel,
    math::Vec3Swizzles,
    prelude::*,
    render::texture::ImageSettings,
    utils::HashMap,
};
use bevy_ecs_tilemap::tiles::TileTexture;
use bevy_egui::{EguiContext, EguiPlugin};
use bevy_rapier2d::prelude::*;
use burner::{burner_load, burner_tick, Burner};
use character_ui::Building;
use egui::Align2;
use inventory::{drop_between_inventories, inventory_grid, Drag, Source};
use iyes_loopless::prelude::{AppLooplessStateExt, ConditionSet};
use loading::LoadingPlugin;
use recipe_loader::{Recipe, RecipeLoaderPlugin};
use smelter::smelter_tick;
use structure_loader::StructureLoaderPlugin;
use types::{ActiveCraft, CraftingQueue, Powered, Working};

mod burner;
mod character_ui;
mod inventory;
mod loading;
mod player_movement;
mod recipe_loader;
mod smelter;
mod structure_loader;
mod terrain;
mod types;

use crate::character_ui::CharacterUiPlugin;
use crate::inventory::{Inventory, Output};
use crate::player_movement::PlayerMovementPlugin;
use crate::recipe_loader::RecipeAsset;
use crate::terrain::{CursorPos, HoveredTile, TerrainPlugin, COAL, IRON, STONE, TREE};
use crate::types::{AppState, CursorState, GameState, Player};

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
    .add_loopless_state(AppState::Loading)
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
    .add_plugin(CharacterUiPlugin)
    .add_plugin(PlayerMovementPlugin)
    .add_plugin(RecipeLoaderPlugin)
    .add_plugin(StructureLoaderPlugin)
    .add_plugin(LoadingPlugin)
    .add_enter_system(AppState::Running, spawn_player)
    .add_system_set(
        ConditionSet::new()
            .run_in_state(AppState::Running)
            .with_system(camera_zoom)
            .with_system(interact)
            .with_system(interact_completion)
            .with_system(interact_cancel)
            .with_system(interaction_ui)
            .with_system(craft_ui)
            .with_system(craft_ticker)
            .with_system(pick_building)
            .with_system(building_ui)
            .with_system(smelter_tick)
            .with_system(burner_tick)
            .with_system(burner_load)
            .with_system(working_texture)
            .into(),
    )
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

fn building_ui(
    mut commands: Commands,
    mut egui_ctx: ResMut<EguiContext>,
    mut player_query: Query<
        (Entity, &SelectedBuilding, &mut Inventory),
        (With<Player>, Without<Building>),
    >,
    name: Query<&Name>,
    mut building_inventory_query: Query<&mut Inventory, With<Building>>,
    mut crafting_machine_query: Query<
        (&mut Source, &mut Output, &CraftingQueue),
        (With<CraftingQueue>, With<Building>),
    >,
    mut burner_query: Query<&mut Burner, With<Building>>,
    icons: Res<HashMap<String, egui::TextureId>>,
) {
    for (player_entity, SelectedBuilding(selected_building), mut player_inventory) in
        &mut player_query
    {
        let name = name
            .get(*selected_building)
            .map_or("Building", |n| n.as_str());

        let mut window_open = true;
        let mut character_inven_drag = (None, None);
        let mut building_inven_drag = (None, None);
        let mut crafting_source_drag = (None, None);
        let mut crafting_output_drag = (None, None);
        let mut burner_drag = (None, None);
        egui::Window::new(name)
            .resizable(false)
            .open(&mut window_open)
            .show(egui_ctx.ctx_mut(), |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(2., egui::Color32::from_gray(10)))
                        .inner_margin(5.)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label("Character");
                                character_inven_drag =
                                    inventory_grid("character", &mut player_inventory, ui, &icons);
                            });
                        });
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(2., egui::Color32::from_gray(10)))
                        .inner_margin(5.)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                if let Ok(mut inventory) =
                                    building_inventory_query.get_mut(*selected_building)
                                {
                                    building_inven_drag =
                                        inventory_grid("building", &mut inventory, ui, &icons);
                                }
                                if let Ok((input, output, crafting_queue)) =
                                    crafting_machine_query.get_mut(*selected_building)
                                {
                                    (crafting_source_drag, crafting_output_drag) =
                                        crafting_machine_widget(
                                            ui,
                                            &icons,
                                            input,
                                            &crafting_queue,
                                            output,
                                        );
                                }

                                if let Ok(mut burner) = burner_query.get_mut(*selected_building) {
                                    ui.separator();
                                    burner_drag = burner_widget(ui, &icons, &mut burner);
                                }
                            });
                        });
                    if ui.input().pointer.any_released() {
                        // Get a mutable reference to all inventory types and pass them to drop_between_inventories
                        let mut inventories: Vec<(&mut Inventory, Drag)> =
                            vec![(&mut player_inventory, character_inven_drag)];

                        let mut building_inventory =
                            building_inventory_query.get_mut(*selected_building).ok();
                        if let Some(inventory) = &mut building_inventory {
                            inventories.push((inventory, building_inven_drag));
                        }

                        let mut smelter_input =
                            crafting_machine_query.get_mut(*selected_building).ok();
                        if let Some((ref mut input, ref mut output, _)) = &mut smelter_input {
                            inventories.push((&mut input.0, crafting_source_drag));
                            inventories.push((&mut output.0, crafting_output_drag));
                        }

                        let mut burner = burner_query.get_mut(*selected_building).ok();
                        if let Some(burner) = &mut burner {
                            inventories.push((&mut burner.fuel_inventory, burner_drag));
                        }

                        drop_between_inventories(&mut inventories);
                    }
                });
            });

        if !window_open {
            commands.entity(player_entity).remove::<SelectedBuilding>();
        }
    }
}

fn burner_widget(
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
    burner: &mut Burner,
) -> inventory::Drag {
    let mut drag = (None, None);
    ui.horizontal(|ui| {
        ui.label("Fuel:");
        drag = inventory_grid("burner", &mut burner.fuel_inventory, ui, icons);
        if let Some(timer) = &burner.fuel_timer {
            ui.add(egui::ProgressBar::new(1. - timer.percent()).desired_width(100.));
        } else {
            ui.add(egui::ProgressBar::new(0.).desired_width(100.));
        }
    });
    drag
}

fn crafting_machine_widget(
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
    mut source: Mut<Source>,
    crafting_queue: &CraftingQueue,
    mut output: Mut<Output>,
) -> (inventory::Drag, Drag) {
    let mut source_drag = (None, None);
    let mut output_drag = (None, None);
    ui.horizontal_centered(|ui| {
        source_drag = inventory_grid("input", &mut source.0, ui, icons);
        if let Some(active_craft) = crafting_queue.0.front() {
            ui.add(
                egui::ProgressBar::new(active_craft.timer.percent())
                    .desired_width(100.)
                    .show_percentage(),
            );
        } else {
            ui.add(
                egui::ProgressBar::new(0.)
                    .desired_width(100.)
                    .show_percentage(),
            );
        }
        output_drag = inventory_grid("output", &mut output.0, ui, icons);
    });

    (source_drag, output_drag)
}
fn spawn_player(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn_bundle(SpriteBundle {
            texture: asset_server.load("textures/character.png"),
            transform: Transform::from_xyz(0.0, 0.0, 1.0),
            ..Default::default()
        })
        .insert(Player)
        .insert(Inventory {
            slots: vec![None; 12],
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
            commands.entity(entity).remove::<MineCountdown>();
            let tile_entity = interaction.target;
            if let Ok(tile_texture) = tile_query.get(tile_entity) {
                match tile_texture.0 {
                    COAL => inventory.add_item(Resource::Coal, 1),
                    IRON => inventory.add_item(Resource::IronOre, 1),
                    STONE => inventory.add_item(Resource::Stone, 1),
                    TREE => inventory.add_item(Resource::Wood, 1),
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

fn craft_ui(
    mut egui_context: ResMut<EguiContext>,
    blueprints: Res<HashMap<String, Recipe>>,
    mut player_query: Query<(&mut Inventory, &mut CraftingQueue), With<Player>>,
) {
    egui::Window::new("Crafting").show(egui_context.ctx_mut(), |ui| {
        for blueprint in blueprints.values() {
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
