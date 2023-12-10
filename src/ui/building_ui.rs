use bevy::{prelude::*, utils::HashMap};
use bevy_egui::EguiContexts;

use crate::{
    burner::Burner,
    inventory::{Fuel, Inventory, Output, Source},
    loading::{Icons, Resources, Structures},
    picker::SelectedBuilding,
    placeable::Building,
    player::Player,
    types::CraftingQueue,
    ui::inventory_grid::{inventory_grid, Hand, SlotEvent},
    util::{get_inventory_child, FuelInventoryQuery, OutputInventoryQuery, SourceInventoryQuery},
};

pub fn building_ui(
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
        ),
    >,
    name: Query<&Name>,
    mut building_inventory_query: Query<&mut Inventory, With<Building>>,
    source_query: Query<SourceInventoryQuery>,
    output_query: Query<OutputInventoryQuery>,
    fuel_query: Query<FuelInventoryQuery>,
    mut crafting_machine_query: Query<(&CraftingQueue, &Children), With<Building>>,
    mut burner_query: Query<(&mut Burner, &Children), With<Building>>,
    icons: Res<Icons>,
    mut slot_events: EventWriter<SlotEvent>,
    structures: Res<Structures>,
    resources: Res<Resources>,
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
                                    &icons,
                                    hand,
                                    &mut slot_events,
                                    &structures,
                                    &resources,
                                );
                            });
                        });
                    egui::Frame::none()
                        .stroke(egui::Stroke::new(2., egui::Color32::from_gray(10)))
                        .inner_margin(5.)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                if let Ok(inventory) =
                                    building_inventory_query.get_mut(*selected_building)
                                {
                                    inventory_grid(
                                        *selected_building,
                                        &inventory,
                                        ui,
                                        &icons,
                                        hand,
                                        &mut slot_events,
                                        &structures,
                                        &resources,
                                    );
                                }
                                if let Ok((crafting_queue, children)) =
                                    crafting_machine_query.get_mut(*selected_building)
                                {
                                    let source = get_inventory_child(children, &source_query);
                                    let output = get_inventory_child(children, &output_query);

                                    crafting_machine_widget(
                                        ui,
                                        &icons,
                                        crafting_queue,
                                        source,
                                        output,
                                        hand,
                                        &mut slot_events,
                                        &structures,
                                        &resources,
                                    );
                                }

                                if let Ok((burner, children)) =
                                    burner_query.get_mut(*selected_building)
                                {
                                    ui.separator();
                                    let fuel = get_inventory_child(children, &fuel_query);
                                    burner_widget(
                                        ui,
                                        &icons,
                                        &burner,
                                        fuel,
                                        hand,
                                        &mut slot_events,
                                        &structures,
                                        &resources,
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
    resources: &Resources,
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
    resources: &Resources,
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

#[cfg(test)]
mod test {
    use crate::types::Product;

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

        let building_a_id = app.world.spawn((Burner::new(), Building)).id();

        let mut inventory = Inventory::new(1);
        inventory.add_item(Product::Intermediate("Wood".into()), 1);
        let a_child = app.world.spawn((Fuel, inventory)).id();

        app.world
            .entity_mut(building_a_id)
            .push_children(&[a_child]);

        let _building_b_id = app
            .world
            .spawn((Burner::new(), Building))
            .with_children(|b| {
                let mut inventory = Inventory::new(1);
                inventory.add_item(Product::Intermediate("Coal".into()), 1);
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
