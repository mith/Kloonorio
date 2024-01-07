use bevy::{input, prelude::*, utils::HashMap};
use bevy_egui::EguiContexts;

use egui::{epaint, Pos2, Response, Sense, Stroke};

use crate::{
    inventory_grid::{inventory_grid, Hand, SlotEvent, HIGHLIGHT_COLOR},
    util::Definitions,
};
use kloonorio_core::{
    inventory::Inventory,
    player::Player,
    recipe::Recipe,
    types::{ActiveCraft, CraftingQueue},
};

use super::{icon::recipe_icon, tooltip::recipe_tooltip, UiSet};

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

pub fn craft_ui(
    ui: &mut egui::Ui,
    inventory: &mut Inventory,
    build_queue: &mut CraftingQueue,
    definitions: &Definitions,
) {
    let mut recipe_it = definitions.recipes.values();
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
                            .map(|(item, amount)| {
                                let amount_in_inventory = inventory.num_items(item);
                                if amount_in_inventory > 0 {
                                    amount_in_inventory / amount
                                } else {
                                    0
                                }
                            })
                            .min()
                            .unwrap_or(0);
                        let items_available = craftable_amount > 0;
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
                            .add_enabled_ui(items_available, |ui| {
                                recipe_slot(ui, recipe, craftable_amount, &definitions.icons);
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
                            recipe_tooltip(ui, recipe, definitions);
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

#[derive(Resource, Default)]
struct CharacterUiOpen(bool);

fn toggle_character_ui(
    mut character_ui_open: ResMut<CharacterUiOpen>,
    input: Res<Input<input::keyboard::KeyCode>>,
) {
    if input.just_pressed(KeyCode::Period) {
        character_ui_open.0 = !character_ui_open.0;
    }
}

fn character_ui(
    mut egui_context: EguiContexts,
    mut inventory_query: Query<(Entity, &mut Inventory, &Hand, &mut CraftingQueue), With<Player>>,
    mut slot_events: EventWriter<SlotEvent>,
    character_ui_open: Res<CharacterUiOpen>,
    definitions: Definitions,
) {
    if !character_ui_open.0 {
        return;
    }

    egui::Window::new("Character")
        .resizable(false)
        .collapsible(false)
        .title_bar(false)
        .show(egui_context.ctx_mut(), |ui| {
            let (player_entity, ref mut inventory, hand, ref mut crafting_queue) =
                inventory_query.single_mut();
            egui::Grid::new("character_ui_grid")
                .spacing([10., 10.])
                .show(ui, |ui| {
                    ui.heading("Character");
                    ui.heading("Crafting");
                    ui.end_row();
                    inventory_grid(
                        player_entity,
                        inventory,
                        ui,
                        hand,
                        &mut slot_events,
                        &definitions,
                    );
                    craft_ui(ui, inventory, crafting_queue, &definitions);
                });
        });
}

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct CharacterUiSet;

pub struct CharacterUiPlugin;
impl Plugin for CharacterUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CharacterUiOpen>().add_systems(
            Update,
            (toggle_character_ui, character_ui)
                .chain()
                .in_set(UiSet)
                .in_set(CharacterUiSet),
        );
    }
}
