use bevy::{prelude::*, utils::HashMap};
use bevy_egui::EguiContext;
use bevy_rapier2d::prelude::*;
use egui::{epaint, Response, Sense, Stroke};

use crate::{
    burner::Burner,
    inventory::{Fuel, Inventory, Output, Source},
    inventory_grid::{
        drop_slot, inventory_grid, item_in_hand, set_drop_slot, set_item_in_hand, Hand, HoverSlot,
        HIGHLIGHT_COLOR,
    },
    smelter::Smelter,
    structure_loader::{Structure, StructureComponent},
    terrain::{HoveredTile, TerrainStage, TILE_SIZE},
    types::{ActiveCraft, CraftingQueue, Player, Recipe, Resource, UiPhase},
};

#[derive(Component)]
pub struct Placeable {
    structure: String,
    size: IVec2,
}

#[derive(Component)]
struct Ghost;

#[derive(Component)]
pub(crate) struct Building;

fn placeable(
    mut commands: Commands,
    mut placeable_query: Query<(Entity, &mut Inventory, &Hand, &HoveredTile)>,
    mouse_input: Res<Input<MouseButton>>,
    ghosts: Query<Entity, With<Ghost>>,
    asset_server: Res<AssetServer>,
    rapier_context: Res<RapierContext>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    structures: Res<HashMap<String, Structure>>,
) {
    // delete old ghosts
    for ghost in ghosts.iter() {
        commands.entity(ghost).despawn_recursive();
    }

    for (player, mut inventory, hand, hovered_tile) in &mut placeable_query {
        if let Some(stack) = &mut inventory.slots[hand.0.slot].as_mut() {
            if let Resource::Structure(structure_name) = &stack.resource {
                let structure = structures.get(structure_name).unwrap();
                let texture_handle = asset_server.load(&format!(
                    "textures/{}.png",
                    &structure.name.to_lowercase().replace(" ", "_")
                ));
                let texture_atlas = TextureAtlas::from_grid(
                    texture_handle,
                    structure.size.as_vec2() * Vec2::new(TILE_SIZE.x, TILE_SIZE.y),
                    2,
                    1,
                );
                let texture_atlas_handle = texture_atlases.add(texture_atlas);
                let translation =
                    hovered_tile.tile_center + Vec2::new(0.5 * TILE_SIZE.x, 0.5 * TILE_SIZE.y);
                let transform = Transform::from_translation(translation.extend(1.0));

                if rapier_context
                    .intersection_with_shape(
                        translation,
                        0.,
                        &Collider::cuboid(16., 16.),
                        QueryFilter::new(),
                    )
                    .is_some()
                {
                    commands
                        .spawn()
                        .insert_bundle(SpriteSheetBundle {
                            transform,
                            texture_atlas: texture_atlas_handle,
                            sprite: TextureAtlasSprite {
                                color: Color::rgba(1.0, 0.3, 0.3, 0.5),
                                ..default()
                            },
                            ..default()
                        })
                        .insert(Ghost);
                } else if mouse_input.just_pressed(MouseButton::Left) {
                    info!("Placing {:?}", structure);
                    commands.entity(player).remove::<Placeable>();
                    if inventory.remove_items(&[(Resource::Structure(structure.name.clone()), 1)]) {
                        let structure_entity = commands
                            .spawn()
                            .insert_bundle(SpriteSheetBundle {
                                texture_atlas: texture_atlas_handle,
                                ..default()
                            })
                            .insert(Collider::cuboid(11., 11.))
                            .insert_bundle(TransformBundle::from_transform(transform))
                            .insert(Building)
                            .insert(Name::new(structure.name.to_string()))
                            .id();

                        for component in &structure.components {
                            match component {
                                StructureComponent::Smelter => {
                                    commands.entity(structure_entity).insert(Smelter);
                                }
                                StructureComponent::Burner => {
                                    commands.entity(structure_entity).insert(Burner::new());
                                }
                                StructureComponent::CraftingQueue => {
                                    commands
                                        .entity(structure_entity)
                                        .insert(CraftingQueue::default());
                                }
                                StructureComponent::Inventory(slots) => {
                                    commands
                                        .entity(structure_entity)
                                        .insert(Inventory::new(*slots));
                                }
                                StructureComponent::Source(slots) => {
                                    commands.entity(structure_entity).with_children(|p| {
                                        p.spawn().insert(Source).insert(Inventory::new(*slots));
                                    });
                                }
                                StructureComponent::Output(slots) => {
                                    commands.entity(structure_entity).with_children(|p| {
                                        p.spawn().insert(Output).insert(Inventory::new(*slots));
                                    });
                                }
                                StructureComponent::Fuel(slots) => {
                                    commands.entity(structure_entity).with_children(|p| {
                                        p.spawn().insert(Fuel).insert(Inventory::new(*slots));
                                    });
                                }
                            }
                        }
                    }
                } else {
                    commands
                        .spawn()
                        .insert_bundle(SpriteSheetBundle {
                            transform,
                            texture_atlas: texture_atlas_handle,
                            sprite: TextureAtlasSprite {
                                color: Color::rgba(1.0, 1.0, 1.0, 0.5),
                                ..default()
                            },
                            ..default()
                        })
                        .insert(Ghost);
                }
            }
        }
    }
}

pub fn recipe_icon(
    ui: &mut egui::Ui,
    recipe: &Recipe,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    let icon_name = &recipe.name.to_lowercase().replace(" ", "_");
    let response = {
        if let Some(egui_img) = icons.get(icon_name) {
            ui.image(*egui_img, [32., 32.])
        } else if let Some(no_icon_img) = icons.get("no_icon") {
            ui.image(*no_icon_img, [32., 32.])
        } else {
            ui.label("NO ICON")
        }
    };
    response
}

pub fn craft_ui(
    ui: &mut egui::Ui,
    recipes: &HashMap<String, Recipe>,
    inventory: &mut Inventory,
    build_queue: &mut CraftingQueue,
    icons: &HashMap<String, egui::TextureId>,
) {
    let mut recipe_it = recipes.values();
    egui::Grid::new("crafting")
        .min_col_width(32.)
        .max_col_width(32.)
        .spacing([3., 3.])
        .show(ui, |ui| {
            for _ in 0..10 {
                for _ in 0..10 {
                    if let Some(recipe) = recipe_it.next() {
                        let resources_available = inventory.has_items(&recipe.materials);
                        let response = ui.add_enabled_ui(resources_available, |ui| {
                            let (rect, response) = ui.allocate_exact_size(
                                egui::Vec2::new(32., 32.),
                                Sense::hover().union(Sense::click()),
                            );
                            let (style, bg_fill) = if response.hovered() {
                                (ui.visuals().widgets.active, HIGHLIGHT_COLOR)
                            } else {
                                (ui.visuals().widgets.inactive, egui::Color32::from_gray(40))
                            };
                            ui.painter().add(epaint::RectShape {
                                rounding: style.rounding,
                                fill: bg_fill,
                                stroke: Stroke::none(),
                                rect,
                            });
                            ui.child_ui(rect, *ui.layout())
                                .add_enabled_ui(resources_available, |ui| {
                                    recipe_icon(ui, recipe, icons)
                                });
                            response
                        });
                        if response.inner.clicked() {
                            inventory.remove_items(&recipe.materials);
                            build_queue.0.push_back(ActiveCraft {
                                blueprint: recipe.clone(),
                                timer: Timer::from_seconds(recipe.crafting_time, false),
                            });
                        }
                    } else {
                        let (_id, rect) = ui.allocate_space(egui::Vec2::new(32., 32.));
                        ui.painter().add(epaint::RectShape {
                            rounding: egui::Rounding::none(),
                            fill: egui::Color32::from_gray(40),
                            stroke: Stroke::none(),
                            rect,
                        });
                    }
                }
                ui.end_row();
            }
        });
}

fn character_ui(
    mut commands: Commands,
    mut egui_context: ResMut<EguiContext>,
    mut inventory_query: Query<(Entity, &mut Inventory, &mut CraftingQueue), With<Player>>,
    blueprints: Res<HashMap<String, Recipe>>,
    icons: Res<HashMap<String, egui::TextureId>>,
    hand_query: Query<&Hand>,
) {
    egui::Window::new("Character")
        .resizable(false)
        .show(egui_context.ctx_mut(), |ui| {
            for (player_entity, ref mut inventory, ref mut crafting_queue) in &mut inventory_query {
                set_item_in_hand(ui, hand_query.get(player_entity).ok().cloned());
                set_drop_slot(ui, None);

                ui.horizontal_top(|ui| {
                    let drag = inventory_grid(player_entity, inventory, ui, &icons);
                    ui.separator();
                    craft_ui(ui, &blueprints, inventory, crafting_queue, &icons);
                    drag
                });

                if let Some(hand) = item_in_hand(ui) {
                    commands.entity(player_entity).remove::<Hand>().insert(hand);
                } else {
                    commands.entity(player_entity).remove::<Hand>();
                }

                if let Some(hover_slot) = drop_slot(ui) {
                    commands
                        .entity(player_entity)
                        .remove::<HoverSlot>()
                        .insert(hover_slot);
                } else {
                    commands.entity(player_entity).remove::<HoverSlot>();
                }
            }
        });
}

#[derive(SystemLabel)]
pub struct CharacterUiPhase;

pub struct CharacterUiPlugin;
impl Plugin for CharacterUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::new()
                .label(UiPhase)
                .label(CharacterUiPhase)
                .after(TerrainStage)
                .with_system(character_ui)
                .with_system(placeable)
                .into(),
        );
    }
}
