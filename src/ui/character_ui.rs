use bevy::{prelude::*, utils::HashMap};
use bevy_egui::EguiContexts;

use egui::{epaint, Color32, Pos2, Response, RichText, Sense, Stroke};

use crate::{
    inventory::{Inventory, Stack},
    loading::{Icons, Recipes},
    player::Player,
    terrain::TerrainSet,
    types::{ActiveCraft, CraftingQueue, Recipe},
    ui::inventory_grid::{inventory_grid, Hand, SlotEvent, HIGHLIGHT_COLOR},
};

use super::{inventory_grid::resource_icon, UiSet};

pub fn recipe_slot(
    ui: &mut egui::Ui,
    recipe: &Recipe,
    craftable_amount: u32,
    icons: &HashMap<String, egui::TextureId>,
) -> Response {
    let response = recipe_icon(ui, recipe, icons);

    if craftable_amount > 0 {
        let font_id = egui::FontId::proportional(16.);
        let layout = ui.fonts(|fonts| {
            fonts.layout_no_wrap(craftable_amount.to_string(), font_id, egui::Color32::WHITE)
        });
        let rect = response.rect;
        let pos = Pos2::new(
            rect.right() - layout.size().x - 1.,
            rect.bottom() - layout.size().y - 1.,
        );
        ui.painter().add(epaint::TextShape {
            pos,
            galley: layout,
            underline: Stroke::new(1., egui::Color32::BLACK),
            override_text_color: None,
            angle: 0.,
        });
    }
    response
}

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
                        let craftable_amount = recipe
                            .ingredients
                            .iter()
                            .map(|(resource, amount)| {
                                let amount_in_inventory = inventory.num_items(resource);
                                if amount_in_inventory > 0 {
                                    amount_in_inventory / amount
                                } else {
                                    0
                                }
                            })
                            .min()
                            .unwrap_or(0);
                        let resources_available = craftable_amount > 0;
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
                                recipe_slot(ui, recipe, craftable_amount, icons);
                            });

                        if response.clicked() {
                            inventory.remove_items(&recipe.ingredients);
                            build_queue.0.push_back(ActiveCraft {
                                recipe: recipe.clone(),
                                timer: Timer::from_seconds(
                                    recipe.crafting_time,
                                    TimerMode::Repeating,
                                ),
                            });
                        }
                        response.on_hover_ui_at_pointer(|ui| {
                            recipe_tooltip(ui, recipe, icons);
                        });
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

pub fn recipe_tooltip(
    ui: &mut egui::Ui,
    recipe: &Recipe,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    egui::Grid::new("recipe_info")
        .spacing([3., 3.])
        .with_row_color(|row, _style| {
            if row == 0 {
                Some(Color32::from_gray(200))
            } else {
                None
            }
        })
        .show(ui, |ui| {
            ui.label(
                RichText::new(recipe.name.clone() + " (Recipe)")
                    .heading()
                    .color(Color32::BLACK),
            );
            ui.end_row();
            ui.label("Ingredients:");
            ui.end_row();
            for (ingredient, amount) in &recipe.ingredients {
                ui.horizontal(|ui| {
                    resource_icon(ui, &Stack::new(ingredient.clone(), *amount), icons);
                    ui.label(format!("{} x {}", amount, ingredient));
                });
                ui.end_row();
            }
            ui.end_row();
            ui.label(format!("Crafting time: {}s", recipe.crafting_time));
        })
        .response
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
