use bevy::ecs::{query::With, system::Query};
use bevy_egui::EguiContexts;
use egui::{epaint::Shadow, Align2, Color32, Frame};
use kloonorio_core::{health::Health, player::Player};

pub fn healthbar(
    mut egui_context: EguiContexts,
    player_health_query: Query<&Health, With<Player>>,
) {
    let Ok(player_health) = player_health_query.get_single() else {
        return;
    };

    if player_health.current == player_health.max {
        return;
    }

    let health_percent = player_health.current as f32 / player_health.max as f32;
    let color = if health_percent > 0.5 {
        Color32::GREEN
    } else if health_percent > 0.25 {
        Color32::YELLOW
    } else {
        Color32::RED
    };
    egui::Area::new("Player Health")
        .movable(false)
        .anchor(Align2::CENTER_BOTTOM, (0., -52.))
        .interactable(false)
        .show(egui_context.ctx_mut(), |ui| {
            Frame::window(ui.style())
                .shadow(Shadow::small_light())
                .show(ui, |ui| {
                    ui.add(
                        egui::ProgressBar::new(health_percent)
                            .fill(color)
                            .desired_width(250.),
                    );
                });
        });
}
