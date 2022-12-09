use bevy::{ecs::query::ReadOnlyWorldQuery, prelude::*, utils::HashMap};
use bevy_egui::EguiContext;

use crate::{
    burner::Burner,
    inventory::{Fuel, Inventory, Output, Source},
    inventory_grid::{inventory_grid, Hand, SlotEvent},
    loading::Icons,
    placeable::Building,
    types::{CraftingQueue, Player},
    util::{FuelInventoryQuery, InventoryQuery, OutputInventoryQuery, SourceInventoryQuery},
    SelectedBuilding,
};

pub fn building_ui(
    mut commands: Commands,
    mut egui_ctx: ResMut<EguiContext>,
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

/// Get the inventory of a child entity.
/// Returns a tuple of the child entity and the inventory.
fn get_inventory_child<'b, I>(
    children: &Children,
    output_query: &'b Query<InventoryQuery<I>>,
) -> (Entity, &'b Inventory)
where
    I: ReadOnlyWorldQuery,
{
    let output = children
        .iter()
        .flat_map(|c| output_query.get(*c).map(|i| (*c, i.inventory)))
        .next()
        .unwrap();
    output
}

fn burner_widget(
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
    burner: &Burner,
    fuel: (Entity, &Inventory),
    hand: &Hand,
    slot_events: &mut EventWriter<SlotEvent>,
) {
    ui.horizontal(|ui| {
        ui.label("Fuel:");
        inventory_grid(fuel.0, fuel.1, ui, icons, hand, slot_events);
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
) {
    ui.horizontal_centered(|ui| {
        inventory_grid(source.0, source.1, ui, icons, hand, slot_events);
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
        inventory_grid(output.0, output.1, ui, icons, hand, slot_events);
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

        let mut a_child: Option<Entity> = None;

        let building_a_id = app
            .world
            .spawn((Burner::new(), Building))
            .with_children(|a| {
                let mut inventory = Inventory::new(1);
                inventory.add_item(Product::Intermediate("Wood".into()), 1);
                a_child = Some(a.spawn((Fuel, inventory)).id());
            })
            .id();

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

        app.add_system(test_system);
        app.update();

        let result = app.world.resource::<Result>();

        assert_eq!(result.0, a_child.unwrap());
    }
}
