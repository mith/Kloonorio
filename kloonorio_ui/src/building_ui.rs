use bevy::{ecs::system::SystemParam, prelude::*, utils::HashMap};
use bevy_egui::EguiContexts;

use kloonorio_core::{
    inventory::{
        util::{get_inventory_child, try_get_inventory_child},
        Fuel, Inventory, InventoryParams, Output, Source, Storage,
    },
    item::Items,
    player::Player,
    recipe::Recipes,
    structure::Structures,
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
    crafting_machine_query: Query<'w, 's, (&'static CraftingQueue, &'static Children)>,
    burner_query: Query<'w, 's, (&'static mut Burner, &'static Children)>,
    assembler_query: Query<'w, 's, &'static Assembler>,
    assembler_recipe_change_events: EventWriter<'w, ChangeAssemblerRecipeEvent>,
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
    children: Query<&Children>,
    inventory_params: InventoryParams,
    mut building_param: BuildingParam,
    mut slot_events: EventWriter<SlotEvent>,
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
                                    &definitions.icons,
                                    hand,
                                    &mut slot_events,
                                    &definitions.structures,
                                    &definitions.items,
                                );
                            });
                        });
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(2., egui::Color32::from_gray(10)))
                        .inner_margin(5.)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                if let Some((inventory_child, inventory)) =
                                    children.get(*selected_building).ok().and_then(|c| {
                                        try_get_inventory_child(
                                            c,
                                            &inventory_params.storage_inventories,
                                        )
                                    })
                                {
                                    inventory_grid(
                                        inventory_child,
                                        inventory,
                                        ui,
                                        &definitions.icons,
                                        hand,
                                        &mut slot_events,
                                        &definitions.structures,
                                        &definitions.items,
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
                                if let Ok((crafting_queue, children)) = building_param
                                    .crafting_machine_query
                                    .get_mut(*selected_building)
                                {
                                    let source = get_inventory_child(
                                        children,
                                        &inventory_params.source_inventories,
                                    );
                                    let output = get_inventory_child(
                                        children,
                                        &inventory_params.output_inventories,
                                    );

                                    crafting_machine_widget(
                                        ui,
                                        &definitions.icons,
                                        crafting_queue,
                                        source,
                                        output,
                                        hand,
                                        &mut slot_events,
                                        &definitions.structures,
                                        &definitions.items,
                                    );
                                }

                                if let Ok((burner, children)) =
                                    building_param.burner_query.get_mut(*selected_building)
                                {
                                    ui.separator();
                                    let fuel = get_inventory_child(
                                        children,
                                        &inventory_params.fuel_inventories,
                                    );
                                    burner_widget(
                                        ui,
                                        &definitions.icons,
                                        &burner,
                                        fuel,
                                        hand,
                                        &mut slot_events,
                                        &definitions.structures,
                                        &definitions.items,
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
    icons: &HashMap<String, egui::TextureId>,
    burner: &Burner,
    fuel: (Entity, &Inventory),
    hand: &Hand,
    slot_events: &mut EventWriter<SlotEvent>,
    structures: &Structures,
    resources: &Items,
) {
    ui.horizontal(|ui| {
        ui.label("Fuel:");
        inventory_grid(
            fuel.0,
            fuel.1,
            ui,
            icons,
            hand,
            slot_events,
            structures,
            resources,
        );
        if let Some(timer) = &burner.fuel_timer {
            ui.add(egui::ProgressBar::new(1. - timer.percent()).desired_width(100.));
        } else {
            ui.add(egui::ProgressBar::new(0.).desired_width(100.));
        }
    });
}

fn crafting_machine_widget(
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
    crafting_queue: &CraftingQueue,
    source: (Entity, &Inventory),
    output: (Entity, &Inventory),
    hand: &Hand,
    slot_events: &mut EventWriter<SlotEvent>,
    structures: &Structures,
    resources: &Items,
) {
    ui.horizontal_centered(|ui| {
        inventory_grid(
            source.0,
            source.1,
            ui,
            icons,
            hand,
            slot_events,
            structures,
            resources,
        );
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
        inventory_grid(
            output.0,
            output.1,
            ui,
            icons,
            hand,
            slot_events,
            structures,
            resources,
        );
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

#[cfg(test)]
mod test {
    use kloonorio_core::{inventory::inventory_params::FuelInventoryQuery, item::Item};

    use super::*;

    #[derive(Resource)]
    struct Target(Entity);

    #[derive(Resource)]
    struct Result(Entity);

    fn test_system(
        mut commands: Commands,
        mut burner_query: Query<(&mut Burner, &Children), With<Building>>,
        fuel_query: Query<FuelInventoryQuery>,
        target_entity: Res<Target>,
    ) {
        if let Ok((mut _burner, children)) = burner_query.get_mut(target_entity.0) {
            let fuel = get_inventory_child(children, &fuel_query);
            commands.insert_resource(Result(fuel.0));
        }
    }

    #[test]
    fn get_inventory_child_only_own() {
        let mut app = App::new();

        let building_a_id = app.world.spawn((Burner::default(), Building)).id();

        let mut inventory = Inventory::new(1);
        inventory.add_item(&Item::new("Wood"), 1);
        let a_child = app.world.spawn((Fuel, inventory)).id();

        app.world
            .entity_mut(building_a_id)
            .push_children(&[a_child]);

        let _building_b_id = app
            .world
            .spawn((Burner::default(), Building))
            .with_children(|b| {
                let mut inventory = Inventory::new(1);
                inventory.add_item(&Item::new("Coal"), 1);
                b.spawn((Fuel, inventory));
            })
            .id();

        // Add target entity
        app.insert_resource(Target(building_a_id));

        app.add_systems(Update, test_system);
        app.update();

        let result = app.world.resource::<Result>();

        assert_eq!(result.0, a_child);
    }
}
