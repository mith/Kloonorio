use std::ops::{Deref, DerefMut};

use bevy::{ecs::system::Resource, utils::HashMap};
use egui::Response;

use kloonorio_core::{inventory::Stack, recipe::Recipe};

#[derive(Resource, Default)]
pub struct Icons(HashMap<String, egui::TextureId>);

impl Deref for Icons {
    type Target = HashMap<String, egui::TextureId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Icons {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
pub fn stack_icon(
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
