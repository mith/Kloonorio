use egui::Response;

use crate::{inventory::Stack, types::Recipe};

pub fn resource_icon(
    ui: &mut egui::Ui,
    stack: &Stack,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    let icon_name = &stack.item.to_string().to_lowercase().replace(' ', "_");
    let response = {
        if let Some(egui_img) = icons.get(icon_name) {
            ui.image((*egui_img, egui::Vec2::new(32., 32.)))
        } else if let Some(no_icon_img) = icons.get("no_icon") {
            ui.image((*no_icon_img, egui::Vec2::new(32., 32.)))
        } else {
            ui.label("NO ICON")
        }
    };
    response
}

pub fn recipe_icon(
    ui: &mut egui::Ui,
    recipe: &Recipe,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    item_icon(ui, recipe.products[0].0.as_ref(), icons)
}

pub fn item_icon(
    ui: &mut egui::Ui,
    name: &str,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    let icon_name = &name.to_lowercase().replace(' ', "_");
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
