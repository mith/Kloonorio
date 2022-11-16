use crate::character_ui::CharacterUiPlugin;
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
use inventory::{drop_within_inventory, transfer_between_slots, Source};
use inventory_grid::{inventory_grid, Hand, HoverSlot};
use iyes_loopless::prelude::{AppLooplessStateExt, ConditionSet};
use loading::LoadingPlugin;
use recipe_loader::RecipeLoaderPlugin;
use smelter::smelter_tick;
use structure_loader::StructureLoaderPlugin;
use types::{CraftingQueue, Powered, Working};

mod burner;
mod character_ui;
mod inventory;
mod inventory_grid;
mod loading;
mod player_movement;
mod recipe_loader;
mod smelter;
mod structure_loader;
mod terrain;
mod types;

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
            .with_system(craft_ticker)
            .with_system(pick_building)
            .with_system(building_ui)
            .with_system(smelter_tick)
            .with_system(burner_tick)
            .with_system(burner_load)
            .with_system(working_texture)
            .with_system(drop_system)
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

fn drop_system(
    mut commands: Commands,
    mut hand_query: Query<(&mut Hand, &mut HoverSlot, Entity), With<Player>>,
    mut inventories_query: Query<&mut Inventory>,
    mouse_input: Res<Input<MouseButton>>,
) {
    if mouse_input.just_pressed(MouseButton::Left) {
        for (hand, drop, entity) in &mut hand_query {
            if hand.0.entity == drop.0.entity {
                let mut inventory = inventories_query.get_mut(hand.0.entity).unwrap();
                drop_within_inventory(&mut inventory, hand.0.slot, drop.0.slot);
            } else if let Ok([mut source_inventory, mut target_inventory]) =
                inventories_query.get_many_mut([hand.0.entity, drop.0.entity])
            {
                transfer_between_slots(
                    source_inventory.slots.get_mut(hand.0.slot).unwrap(),
                    target_inventory.slots.get_mut(drop.0.slot).unwrap(),
                );
            }

            commands
                .entity(entity)
                .remove::<Hand>()
                .remove::<HoverSlot>();
        }
    }
}

// fn ghost_placeable() {
//
//     if let Some(stack) = inventories_query.get_single.slots[hand.0.slot].clone() {
//         if let Resource::Structure(structure_name) = stack.resource {
//             if let Ok(player) = player_query.get_single() {
//                 let structure = structures.get(&structure_name).unwrap();
//                 commands.entity(player).insert(Placeable {
//                     structure: structure_name.clone(),
//                     size: structure.size,
//                 });
//             }
//         }
//     }
// // }
//  let Some(hand) = item_in_hand(ui).clone() {
//                     if !ui.ui_contains_pointer() {
//                         if let Some(stack) = inventory.slots[hand.0.slot].clone() {
//                             if let Resource::Structure(structure_name) = stack.resource {
//                                 if let Ok(player) = player_query.get_single() {
//                                     let structure = structures.get(&structure_name).unwrap();
//                                     commands.entity(player).insert(Placeable {
//                                             structure: structure_name.clone(),
//                                             size: structure.size,
//                                         });
//                                 }
//                             }
//                         }
//                     } else {
//                         commands
//                             .entity(player_query.get_single().unwrap())
//                                 .remove::<Placeable>();
//                     }
//                 }
//             }

fn building_ui(
    mut commands: Commands,
    mut egui_ctx: ResMut<EguiContext>,
    mut player_query: Query<
        (Entity, &SelectedBuilding, &mut Inventory),
        (
            With<Player>,
            Without<Building>,
            Without<Source>,
            Without<Output>,
        ),
    >,
    name: Query<&Name>,
    mut building_inventory_query: Query<&mut Inventory, With<Building>>,
    source_query: Query<&mut Inventory, (With<Source>, Without<Output>, Without<Building>)>,
    output_query: Query<&mut Inventory, (With<Output>, Without<Source>, Without<Building>)>,
    mut crafting_machine_query: Query<(&CraftingQueue, &Children), With<Building>>,
    mut burner_query: Query<(&mut Burner, &Children), With<Building>>,
    icons: Res<HashMap<String, egui::TextureId>>,
    mut hand_query: Query<(&mut Hand, &mut HoverSlot)>,
) {
    if let Ok((player_entity, SelectedBuilding(selected_building), mut player_inventory)) =
        player_query.get_single()
    {
        let name = name
            .get(*selected_building)
            .map_or("Building", |n| n.as_str());

        let mut window_open = true;
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
                                inventory_grid(player_entity, &mut player_inventory, ui, &icons);
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
                                    inventory_grid(*selected_building, &mut inventory, ui, &icons);
                                }
                                if let Ok((crafting_queue, children)) =
                                    crafting_machine_query.get_mut(*selected_building)
                                {
                                    let source = children
                                        .iter()
                                        .flat_map(|c| source_query.get(*c).map(|i| (*c, i)))
                                        .next()
                                        .unwrap();
                                    let output = children
                                        .iter()
                                        .flat_map(|c| output_query.get(*c).map(|i| (*c, i)))
                                        .next()
                                        .unwrap();

                                    crafting_machine_widget(
                                        ui,
                                        &icons,
                                        *selected_building,
                                        &crafting_queue,
                                        source,
                                        output,
                                    );
                                }

                                if let Ok((burner, children)) =
                                    burner_query.get_mut(*selected_building)
                                {
                                    ui.separator();
                                    let fuel = children
                                        .iter()
                                        .flat_map(|c| source_query.get(*c).map(|i| (*c, i)))
                                        .next()
                                        .unwrap();
                                    burner_widget(ui, &icons, *selected_building, &burner, fuel);
                                }
                            });
                        });
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
    entity: Entity,
    burner: &Burner,
    fuel: (Entity, &Inventory),
) {
    ui.horizontal(|ui| {
        ui.label("Fuel:");
        inventory_grid(fuel.0, fuel.1, ui, icons);
        if let Some(timer) = &burner.fuel_timer {
            ui.add(egui::ProgressBar::new(1. - timer.percent()).desired_width(100.));
        } else {
            ui.add(egui::ProgressBar::new(0.).desired_width(100.));
        }
    });
}

fn crafting_machine_widget(
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
    entity: Entity,
    crafting_queue: &CraftingQueue,
    source: (Entity, &Inventory),
    output: (Entity, &Inventory),
) {
    ui.horizontal_centered(|ui| {
        inventory_grid(source.0, source.1, ui, icons);
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
        inventory_grid(output.0, output.1, ui, icons);
    });
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
            slots: vec![None; 100],
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
