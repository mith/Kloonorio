use bevy::{prelude::*, utils::HashMap};
use bevy_egui::EguiContexts;

use egui::{epaint, Response, Sense, Stroke};

use crate::{
    inventory::Inventory,
    loading::{Icons, Recipes},
    player::Player,
    terrain::TerrainSet,
    types::{ActiveCraft, CraftingQueue, Recipe, UiSet},
    ui::inventory_grid::{inventory_grid, Hand, SlotEvent, HIGHLIGHT_COLOR},
};

pub fn recipe_icon(
    ui: &mut egui::Ui,
    recipe: &Recipe,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    let icon_name = &recipe.name.to_lowercase().replace(" ", "_");
    let response = {
        if let Some(egui_img) = icons.get(icon_name) {
            ui.add(egui::Image::new((*egui_img, egui::Vec2::new(32., 32.))))
        } else if let Some(no_icon_img) = icons.get("no_icon") {
            ui.add(egui::Image::new((*no_icon_img, egui::Vec2::new(32., 32.))))
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
                                stroke: Stroke::NONE,
                                rect,
                                fill_texture_id: egui::TextureId::Managed(0),
                                uv: egui::Rect::ZERO,
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
                                timer: Timer::from_seconds(
                                    recipe.crafting_time,
                                    TimerMode::Repeating,
                                ),
                            });
                        }
                    } else {
                        let (_id, rect) = ui.allocate_space(egui::Vec2::new(32., 32.));
                        ui.painter().add(epaint::RectShape {
                            rounding: egui::Rounding::ZERO,
                            fill: egui::Color32::from_gray(40),
                            stroke: Stroke::NONE,
                            rect,
                            fill_texture_id: egui::TextureId::Managed(0),
                            uv: egui::Rect::ZERO,
                        });
                    }
                }
                ui.end_row();
            }
        });
}

fn character_ui(
    mut egui_context: EguiContexts,
    mut inventory_query: Query<(Entity, &mut Inventory, &Hand, &mut CraftingQueue), With<Player>>,
    blueprints: Res<Recipes>,
    icons: Res<Icons>,
    mut slot_events: EventWriter<SlotEvent>,
) {
    egui::Window::new("Character")
        .resizable(false)
        .show(egui_context.ctx_mut(), |ui| {
            for (player_entity, ref mut inventory, hand, ref mut crafting_queue) in
                &mut inventory_query
            {
                ui.horizontal_top(|ui| {
                    inventory_grid(player_entity, inventory, ui, &icons, hand, &mut slot_events);
                    ui.separator();
                    craft_ui(ui, &blueprints, inventory, crafting_queue, &icons);
                });
            }
        });
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharacterUiSet;

pub struct CharacterUiPlugin;
impl Plugin for CharacterUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            character_ui
                .in_set(UiSet)
                .in_set(CharacterUiSet)
                .after(TerrainSet),
        );
    }
}
