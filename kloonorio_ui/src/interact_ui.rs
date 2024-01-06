use bevy::ecs::system::Query;
use bevy_egui::EguiContexts;
use egui::Align2;
use kloonorio_core::types::MineCountdown;

pub fn interaction_ui(mut egui_context: EguiContexts, interact_query: Query<&MineCountdown>) {
    if let Ok(interact) = interact_query.get_single() {
        egui::Window::new("Interaction")
            .anchor(Align2::CENTER_BOTTOM, (0., -65.))
            .title_bar(false)
            .resizable(false)
            .show(egui_context.ctx_mut(), |ui| {
                ui.add(egui::ProgressBar::new(interact.timer.percent()));
            });
    }
}
