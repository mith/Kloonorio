use bevy::ecs::{
    query::With,
    system::{Query, Res},
};
use bevy_egui::EguiContexts;
use egui::{Align2, Color32, PointerButton, Response, Sense};

use crate::{
    loading::{Icons, Items, Structures},
    player::Player,
    types::CraftingQueue,
};

use super::{icon::recipe_icon, tooltip::recipe_tooltip};

pub fn crafting_queue_ui(
    mut egui_context: EguiContexts,
    mut crafting_queue_query: Query<&mut CraftingQueue, With<Player>>,
    icons: Res<Icons>,
    structures: Res<Structures>,
    resources: Res<Items>,
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
                        let response = queue_item_ui(ui, build, &icons);
                        if response.clicked_by(PointerButton::Secondary) {
                            to_cancel.push(index);
                        }
                        if response.hovered() {
                            response.on_hover_ui_at_pointer(|ui| {
                                recipe_tooltip(ui, &build.recipe, &icons, &structures, &resources);
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

fn queue_item_ui(
    ui: &mut egui::Ui,
    build: &mut crate::types::ActiveCraft,
    icons: &Icons,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::Vec2::new(32., 32.),
        Sense::hover().union(Sense::click()),
    );
    if ui.is_rect_visible(rect) {
        ui.child_ui(rect, *ui.layout()).add(|ui: &mut egui::Ui| {
            egui::Frame::none()
                .fill(egui::Color32::GRAY)
                .show(ui, |ui| recipe_icon(ui, &build.recipe, icons))
                .response
        });

        let progress_pct = build.timer.percent();
        let rect = egui::Rect::from_min_size(
            rect.min,
            egui::Vec2::new(rect.width() * progress_pct, rect.height()),
        );
        ui.painter()
            .rect_filled(rect, 0., Color32::from_black_alpha(200));
    }
    response
}
