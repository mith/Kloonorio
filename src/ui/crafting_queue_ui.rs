use bevy::ecs::{
    query::With,
    system::{Query, Res},
};
use bevy_egui::EguiContexts;
use egui::{Align2, PointerButton, Sense};

use crate::{loading::Icons, player::Player, types::CraftingQueue};

use super::character_ui::{recipe_icon, recipe_tooltip};

pub fn crafting_queue_ui(
    mut egui_context: EguiContexts,
    mut crafting_queue_query: Query<&mut CraftingQueue, With<Player>>,
    icons: Res<Icons>,
) {
    let mut to_cancel: Vec<usize> = vec![];
    egui::Area::new("Crafting queue")
        .movable(false)
        .anchor(Align2::LEFT_BOTTOM, (5., -5.))
        .interactable(true)
        .show(egui_context.ctx_mut(), |ui| {
            for mut crafting_queue in &mut crafting_queue_query {
                ui.horizontal(|ui| {
                    for (index, build) in crafting_queue.0.iter_mut().enumerate() {
                        let (rect, response) = ui.allocate_exact_size(
                            egui::Vec2::new(32., 32.),
                            Sense::hover().union(Sense::click()),
                        );
                        ui.child_ui(rect, *ui.layout()).add(|ui: &mut egui::Ui| {
                            egui::Frame::none()
                                .fill(egui::Color32::GRAY)
                                .show(ui, |ui| recipe_icon(ui, &build.recipe, &icons))
                                .response
                        });
                        if response.clicked_by(PointerButton::Secondary) {
                            to_cancel.push(index);
                        }
                        if response.hovered() {
                            response.on_hover_ui_at_pointer(|ui| {
                                recipe_tooltip(ui, &build.recipe, &icons);
                            });
                        }
                    }
                });
            }
        });

    for index in to_cancel.iter().rev() {
        for mut crafting_queue in &mut crafting_queue_query {
            crafting_queue.0.remove(*index);
        }
    }
}
