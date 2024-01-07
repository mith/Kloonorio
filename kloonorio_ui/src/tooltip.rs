use egui::{Color32, Response, RichText};

use kloonorio_core::{
    inventory::Stack, item::Item, item::Items, recipe::Recipe, structure::Structure,
    structure::Structures, structure_components::StructureComponent,
};

use crate::util::Definitions;

use super::icon::stack_icon;

pub fn item_tooltip(
    ui: &mut egui::Ui,
    name: &str,
    structures: &Structures,
    items: &Items,
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
            if let Some(item) = items.get(name) {
                item_rows(ui, item);
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

pub fn item_rows(ui: &mut egui::Ui, item: &Item) {
    let _ = item;
    ui.end_row();
}

pub fn recipe_tooltip(ui: &mut egui::Ui, recipe: &Recipe, definitions: &Definitions) -> Response {
    let structures = &definitions.structures;
    let items = &definitions.items;
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
                    stack_icon(
                        ui,
                        &Stack::new(ingredient.clone(), *amount),
                        &definitions.icons,
                    );
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
                if let Some(item) = items.get(&product.to_string()) {
                    item_rows(ui, item);
                }
                ui.end_row();
            }
        })
        .response
}
