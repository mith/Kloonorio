use bevy::{
    app::{App, Plugin, Update},
    core::Name,
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        system::{Query, Res},
    },
    input::{keyboard::KeyCode, Input},
    reflect::Reflect,
    utils::HashMap,
};
use bevy_egui::EguiContexts;
use egui::{
    epaint::{self, Shadow},
    Align2, Color32, Frame, Pos2, Response, Sense, Stroke,
};

use crate::{
    inventory::Inventory,
    loading::{Icons, Items, Structures},
    player::Player,
};

use super::{icon::item_icon, inventory_grid::Hand, tooltip::item_tooltip};

pub struct HotbarPlugin;

impl Plugin for HotbarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (hotbar_ui, hotbar_keyboard));
    }
}

#[derive(Clone, Debug, Reflect)]
pub struct HotbarItem {
    item: Option<Name>,
}

impl HotbarItem {
    fn clear(&mut self) {
        self.item = None;
    }
}

#[derive(Component, Debug, Reflect)]
pub struct Hotbar(Vec<HotbarItem>);

impl Hotbar {
    pub fn new(slots: u8) -> Self {
        Self(vec![HotbarItem { item: None }; slots as usize])
    }
}

fn hotbar_ui(
    mut egui_context: EguiContexts,
    mut hotbar_query: Query<(Entity, &mut Hotbar, &Inventory, &mut Hand), With<Player>>,
    icons: Res<Icons>,
    structures: Res<Structures>,
    resources: Res<Items>,
) {
    egui::Area::new("Hotbar")
        .movable(false)
        .anchor(Align2::CENTER_BOTTOM, (0., -5.))
        .interactable(true)
        .show(egui_context.ctx_mut(), |ui| {
            for (player_entity, mut hotbar, inventory, mut hand) in &mut hotbar_query {
                Frame::window(ui.style())
                    .shadow(Shadow::small_light())
                    .show(ui, |ui| {
                        egui::Grid::new("crafting")
                            .min_col_width(32.)
                            .max_col_width(32.)
                            .spacing([3., 3.])
                            .show(ui, |ui| {
                                for (index, hotbar_item) in hotbar.0.iter_mut().enumerate() {
                                    // check if the slot contains an item and if the item is in the inventory
                                    let in_inventory = hotbar_item.item.is_some()
                                        && inventory
                                            .find_item(hotbar_item.item.as_ref().unwrap().as_str())
                                            .is_some();
                                    let response = hotbar_item_ui(
                                        ui,
                                        hotbar_item,
                                        &icons,
                                        index as u8,
                                        in_inventory,
                                    );
                                    if response.middle_clicked() {
                                        hotbar_item.clear();
                                    }
                                    if response.clicked() {
                                        if let Some(item) = &hotbar_item.item {
                                            inventory
                                                .find_item(item.as_str())
                                                .map(|index| hand.set_item(player_entity, index));
                                        } else if let Some(inventory_idx) = hand.get_item() {
                                            if let Some(item) = &inventory.slots[inventory_idx.slot]
                                            {
                                                hotbar_item.item =
                                                    Some(Name::new(item.item.to_string()));
                                            }
                                        }
                                    }
                                    if response.hovered() {
                                        if let Some(item) = &hotbar_item.item {
                                            response.on_hover_ui_at_pointer(|ui| {
                                                item_tooltip(
                                                    ui,
                                                    item.as_str(),
                                                    &structures,
                                                    &resources,
                                                );
                                            });
                                        }
                                    }
                                }
                            });
                    });
            }
        });
}

fn hotbar_item_ui(
    ui: &mut egui::Ui,
    item: &mut HotbarItem,
    icons: &Icons,
    index: u8,
    in_inventory: bool,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(32., 32.), Sense::click());
    if ui.is_rect_visible(rect) {
        ui.painter_at(rect)
            .rect_filled(rect, 0., Color32::from_gray(45));
        if let Some(item) = &item.item {
            ui.child_ui(rect, *ui.layout())
                .add_sized(rect.size(), |ui: &mut egui::Ui| {
                    ui.set_enabled(in_inventory);
                    item_icon(ui, item.as_str(), icons)
                });
        }
        let font_id = egui::FontId::proportional(16.);

        // Map of hotbar index to pretty-printed key binding
        let bindings_map = HashMap::from([
            (0, "1"),
            (1, "2"),
            (2, "3"),
            (3, "4"),
            (4, "5"),
            (5, "6"),
            (6, "7"),
            (7, "8"),
            (8, "9"),
            (9, "0"),
        ]);

        let layout = ui.fonts(|fonts| {
            fonts.layout_no_wrap(bindings_map[&index].into(), font_id, egui::Color32::WHITE)
        });
        let rect = response.rect;
        let pos = Pos2::new(
            rect.right() - layout.size().x - 1.,
            rect.bottom() - layout.size().y + 2.,
        );
        ui.painter().add(epaint::TextShape {
            pos,
            galley: layout,
            underline: Stroke::new(1., egui::Color32::BLACK),
            override_text_color: None,
            angle: 0.,
        });
    }
    response
}

fn hotbar_keyboard(
    mut hotbar_query: Query<(Entity, &Hotbar, &mut Hand, &Inventory), With<Player>>,
    keyboard_input: Res<Input<KeyCode>>,
) {
    for (player_entity, hotbar, mut hand, inventory) in &mut hotbar_query {
        let bindings_map = HashMap::from([
            (KeyCode::Key1, 0),
            (KeyCode::Key2, 1),
            (KeyCode::Key3, 2),
            (KeyCode::Key4, 3),
            (KeyCode::Key5, 4),
            (KeyCode::Key6, 5),
            (KeyCode::Key7, 6),
            (KeyCode::Key8, 7),
            (KeyCode::Key9, 8),
            (KeyCode::Key0, 9),
        ]);

        for (key, index) in bindings_map.iter() {
            if keyboard_input.just_pressed(*key) {
                if let Some(item) = &hotbar.0[*index as usize].item {
                    inventory
                        .find_item(item.as_str())
                        .map(|index| hand.set_item(player_entity, index));
                }
            }
        }
    }
}
