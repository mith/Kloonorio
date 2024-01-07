use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_egui::EguiContexts;

use kloonorio_core::{
    inventory::{Fuel, Inventory, InventoryParams, InventoryType, Output, Source, Storage},
    player::Player,
    recipe::Recipes,
    structure_components::{
        assembler::{Assembler, ChangeAssemblerRecipeEvent},
        burner::Burner,
    },
    types::{AppState, Building, CraftingQueue},
};

use crate::{
    inventory_grid::{inventory_grid, Hand, SlotEvent},
    picker::SelectedBuilding,
    util::Definitions,
};

pub struct BuildingUiPlugin;

impl Plugin for BuildingUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, building_ui.run_if(in_state(AppState::Running)));
    }
}

#[derive(SystemParam)]
struct BuildingParam<'w, 's> {
    crafting_machine_query: Query<'w, 's, &'static CraftingQueue>,
    burner_query: Query<'w, 's, &'static mut Burner>,
    assembler_query: Query<'w, 's, &'static Assembler>,
    assembler_recipe_change_events: EventWriter<'w, ChangeAssemblerRecipeEvent>,
    slot_events: EventWriter<'w, SlotEvent>,
}

fn building_ui(
    mut commands: Commands,
    mut egui_ctx: EguiContexts,
    player_query: Query<
        (Entity, &SelectedBuilding, &Inventory, &Hand),
        (
            With<Player>,
            Without<Building>,
            Without<Source>,
            Without<Output>,
            Without<Fuel>,
            Without<Storage>,
        ),
    >,
    name: Query<&Name>,
    inventory_params: InventoryParams,
    mut building_param: BuildingParam,
    definitions: Definitions,
) {
    if let Ok((player_entity, SelectedBuilding(selected_building), player_inventory, hand)) =
        player_query.get_single()
    {
        let name = name
            .get(*selected_building)
            .map_or("Building", |n| n.as_str());

        let mut window_open = true;
        egui::Window::new(name)
            .id(egui::Id::new("building_ui"))
            .resizable(false)
            .open(&mut window_open)
            .show(egui_ctx.ctx_mut(), |ui| {
                ui.horizontal(|ui| {
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(2., egui::Color32::from_gray(10)))
                        .inner_margin(5.)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label("Character");
                                inventory_grid(
                                    player_entity,
                                    player_inventory,
                                    ui,
                                    hand,
                                    &mut building_param.slot_events,
                                    &definitions,
                                );
                            });
                        });
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(2., egui::Color32::from_gray(10)))
                        .inner_margin(5.)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                if let Some((inventory_child, inventory)) = inventory_params
                                    .get_child_inventory(*selected_building, InventoryType::Storage)
                                {
                                    inventory_grid(
                                        inventory_child,
                                        inventory,
                                        ui,
                                        hand,
                                        &mut building_param.slot_events,
                                        &definitions,
                                    );
                                }
                                if let Ok(assembler) =
                                    building_param.assembler_query.get_mut(*selected_building)
                                {
                                    assembling_machine_widget(
                                        ui,
                                        assembler,
                                        *selected_building,
                                        &mut building_param.assembler_recipe_change_events,
                                        &definitions.recipes,
                                    );
                                }
                                if let Ok(crafting_queue) = building_param
                                    .crafting_machine_query
                                    .get_mut(*selected_building)
                                {
                                    let source = inventory_params
                                        .get_child_inventory(
                                            *selected_building,
                                            InventoryType::Source,
                                        )
                                        .unwrap();
                                    let output = inventory_params
                                        .get_child_inventory(
                                            *selected_building,
                                            InventoryType::Output,
                                        )
                                        .unwrap();
                                    crafting_machine_widget(
                                        ui,
                                        crafting_queue,
                                        source,
                                        output,
                                        hand,
                                        &mut building_param.slot_events,
                                        &definitions,
                                    );
                                }

                                if let Ok(burner) =
                                    building_param.burner_query.get_mut(*selected_building)
                                {
                                    ui.separator();
                                    let fuel = inventory_params
                                        .get_child_inventory(
                                            *selected_building,
                                            InventoryType::Fuel,
                                        )
                                        .unwrap();
                                    burner_widget(
                                        ui,
                                        &burner,
                                        fuel,
                                        hand,
                                        &mut building_param.slot_events,
                                        &definitions,
                                    );
                                }
                            });
                        });
                });
            });

        if !window_open {
            commands.entity(player_entity).remove::<SelectedBuilding>();
        }
    }
}

fn burner_widget(
    ui: &mut egui::Ui,
    burner: &Burner,
    fuel: (Entity, &Inventory),
    hand: &Hand,
    slot_events: &mut EventWriter<SlotEvent>,
    definitions: &Definitions,
) {
    ui.horizontal(|ui| {
        ui.label("Fuel:");
        inventory_grid(fuel.0, fuel.1, ui, hand, slot_events, definitions);
        if let Some(timer) = &burner.fuel_timer {
            ui.add(egui::ProgressBar::new(1. - timer.percent()).desired_width(100.));
        } else {
            ui.add(egui::ProgressBar::new(0.).desired_width(100.));
        }
    });
}

fn crafting_machine_widget(
    ui: &mut egui::Ui,
    crafting_queue: &CraftingQueue,
    source: (Entity, &Inventory),
    output: (Entity, &Inventory),
    hand: &Hand,
    slot_events: &mut EventWriter<SlotEvent>,
    definitions: &Definitions,
) {
    ui.horizontal_centered(|ui| {
        inventory_grid(source.0, source.1, ui, hand, slot_events, definitions);
        if let Some(active_craft) = crafting_queue.0.front() {
            ui.add(
                egui::ProgressBar::new(active_craft.timer.percent())
                    .desired_width(100.)
                    .show_percentage(),
            );
        } else {
            ui.add(
                egui::ProgressBar::new(0.)
                    .desired_width(100.)
                    .show_percentage(),
            );
        }
        inventory_grid(output.0, output.1, ui, hand, slot_events, definitions);
    });
}

fn assembling_machine_widget(
    ui: &mut egui::Ui,
    assembler: &Assembler,
    assembler_entity: Entity,
    assembler_recipe_change_events: &mut EventWriter<ChangeAssemblerRecipeEvent>,
    recipes: &Recipes,
) {
    ui.horizontal_centered(|ui| {
        if let Some(recipe) = &assembler.recipe {
            ui.label(recipe.name.as_str());
        }
        ui.menu_button("Select recipe", |ui| {
            for recipe in recipes.values() {
                if ui.button(recipe.name.as_str()).clicked() {
                    assembler_recipe_change_events.send(ChangeAssemblerRecipeEvent {
                        entity: assembler_entity,
                        recipe: recipe.clone(),
                    });
                }
            }
        })
    });
}
