use egui::{Color32, Response, RichText};

use crate::{
    intermediate_loader::Intermediate,
    inventory::Stack,
    loading::{Resources, Structures},
    structure_loader::{Structure, StructureComponent},
    types::Recipe,
};

use super::icon::resource_icon;

pub fn item_tooltip(
    ui: &mut egui::Ui,
    name: &str,
    structures: &Structures,
    resources: &Resources,
) -> Response {
    egui::Grid::new("item_tooltip")
        .spacing([3., 3.])
        .with_row_color(|row, _style| {
            if row == 0 {
                Some(Color32::from_gray(200))
            } else {
                None
            }
        })
        .show(ui, |ui| {
            ui.label(RichText::new(name).heading().color(Color32::BLACK));
            ui.end_row();
            if let Some(structure) = structures.get(name) {
                structure_rows(ui, structure);
            }
            if let Some(resource) = resources.get(name) {
                resource_rows(ui, resource);
            }
        })
        .response
}

pub fn structure_rows(ui: &mut egui::Ui, structure: &Structure) {
    ui.label(format!("Size: {}x{}", structure.size.x, structure.size.y));
    ui.end_row();
    for component in structure.components.iter() {
        match component {
            StructureComponent::Inventory(size) => {
                ui.label(format!("Storage size: {} slots", size));
                ui.end_row();
            }
            StructureComponent::Burner => {
                ui.label("Consumes fuel");
                ui.end_row();
            }
            _ => {}
        }
    }
}

pub fn resource_rows(ui: &mut egui::Ui, resource: &Intermediate) {
    let _ = resource;
    ui.end_row();
}

pub fn recipe_tooltip(
    ui: &mut egui::Ui,
    recipe: &Recipe,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
    structures: &Structures,
    resources: &Resources,
) -> Response {
    egui::Grid::new("recipe_tooltip")
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
            ui.end_row();
            for (product, amount) in &recipe.products {
                if *amount > 1 {
                    ui.strong(format!("Produces: {}({})", product, amount));
                } else {
                    ui.strong(format!("Produces: {}", product));
                }
                ui.end_row();
                if let Some(structure) = structures.get(&product.to_string()) {
                    structure_rows(ui, structure);
                }
                if let Some(resource) = resources.get(&product.to_string()) {
                    resource_rows(ui, resource);
                }
                ui.end_row();
            }
        })
        .response
}
