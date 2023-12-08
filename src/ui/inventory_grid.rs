use bevy::prelude::*;
use bevy::utils::HashMap;
use egui::{
    epaint, Color32, CursorIcon, InnerResponse, Order, Pos2, Response, RichText, Sense, Stroke,
};

use crate::{
    discrete_rotation::DiscreteRotation,
    inventory::{Inventory, Stack},
};

pub const HIGHLIGHT_COLOR: Color32 = egui::Color32::from_rgb(252, 161, 3);

fn item_in_hand(ui: &mut egui::Ui) -> Option<InventoryIndex> {
    let hand_id = egui::Id::new("hand");
    ui.memory(|memory| {
        memory
            .data
            .get_temp::<Hand>(hand_id)
            .and_then(|h| h.item.clone())
    })
}

fn set_hand(ui: &mut egui::Ui, hand: &Hand) {
    let hand_id = egui::Id::new("hand");
    ui.memory_mut(|memory| {
        memory.data.remove::<Hand>(hand_id);
        memory.data.insert_temp::<Hand>(hand_id, hand.clone());
    });
}

fn drag_source(ui: &mut egui::Ui, id: egui::Id, body: impl FnOnce(&mut egui::Ui)) -> Response {
    let is_being_dragged = item_in_hand(ui).filter(|h| h.item_id() == id).is_some();

    if !is_being_dragged {
        let response = ui.scope(body).response;

        let response = ui.interact(response.rect, id, Sense::hover());
        if response.hovered() {
            ui.output_mut(|output| output.cursor_icon = CursorIcon::Grab);
        }
        response
    } else {
        ui.output_mut(|output| output.cursor_icon = CursorIcon::Grabbing);

        // Paint the body to a new layer:
        let layer_id = egui::LayerId::new(Order::Tooltip, id);
        let response = ui.with_layer_id(layer_id, body).response;

        if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
            let delta = pointer_pos - response.rect.center() + egui::Vec2::new(10., 10.);
            ui.ctx().translate_layer(layer_id, delta);
        }
        response
    }
}

fn drop_target<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> InnerResponse<R> {
    let being_dragged = item_in_hand(ui).map_or(false, |h| h.item_id() == id);
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(32., 32.), Sense::click());
    let (style, bg_fill) = if being_dragged || response.hovered() {
        (ui.visuals().widgets.active, HIGHLIGHT_COLOR)
    } else {
        (ui.visuals().widgets.inactive, egui::Color32::from_gray(45))
    };

    ui.painter().add(epaint::RectShape {
        rounding: style.rounding,
        fill: bg_fill,
        stroke: Stroke::NONE,
        rect,
        fill_texture_id: egui::TextureId::Managed(0),
        uv: egui::Rect::ZERO,
    });

    let mut content_ui = ui.child_ui(rect, *ui.layout());
    let ret = body(&mut content_ui);
    InnerResponse::new(ret, response)
}

pub fn item_slot(
    ui: &mut egui::Ui,
    stack: &Stack,
    icons: &HashMap<String, egui::TextureId>,
) -> Response {
    let response = resource_icon(ui, stack, icons);

    let font_id = egui::FontId::proportional(16.);
    let layout = ui.fonts(|fonts| {
        fonts.layout_no_wrap(stack.amount.to_string(), font_id, egui::Color32::WHITE)
    });
    let rect = response.rect;
    let pos = Pos2::new(
        rect.right() - layout.size().x - 1.,
        rect.bottom() - layout.size().y - 1.,
    );
    ui.painter().add(epaint::TextShape {
        pos,
        galley: layout,
        underline: Stroke::new(1., egui::Color32::BLACK),
        override_text_color: None,
        angle: 0.,
    });
    response
}

pub fn resource_icon(
    ui: &mut egui::Ui,
    stack: &Stack,
    icons: &bevy::utils::hashbrown::HashMap<String, egui::TextureId>,
) -> Response {
    let icon_name = &stack.resource.to_string().to_lowercase().replace(" ", "_");
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

pub type SlotIndex = usize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InventoryIndex {
    pub entity: Entity,
    pub slot: SlotIndex,
}

impl InventoryIndex {
    pub fn new(entity: Entity, slot: SlotIndex) -> Self {
        Self { entity, slot }
    }

    pub fn item_id(&self) -> egui::Id {
        egui::Id::new(self.entity).with(self.slot)
    }
}

#[derive(Component, Default, Debug, Clone, PartialEq)]
pub struct Hand {
    pub item: Option<InventoryIndex>,
    pub rotation: Option<DiscreteRotation>,
}

impl Hand {
    #[cfg(test)]
    pub fn new(entity: Entity, slot: SlotIndex) -> Self {
        Self {
            item: Some(InventoryIndex::new(entity, slot)),
            rotation: None,
        }
    }

    pub fn get_item(&self) -> Option<InventoryIndex> {
        self.item.clone()
    }

    pub fn set_item(&mut self, entity: Entity, slot: SlotIndex) {
        self.item = Some(InventoryIndex::new(entity, slot));
    }

    pub fn clear(&mut self) {
        self.item = None;
        self.rotation = None;
    }

    pub fn reset_rotation(&mut self) {
        self.rotation = None;
    }
}

#[derive(Debug, Event)]
pub enum SlotEvent {
    Clicked(InventoryIndex),
}

impl SlotEvent {
    pub fn clicked(entity: Entity, slot: SlotIndex) -> Self {
        Self::Clicked(InventoryIndex::new(entity, slot))
    }
}

pub fn inventory_grid(
    entity: Entity,
    inventory: &Inventory,
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
    hand: &Hand,
    slot_events: &mut EventWriter<SlotEvent>,
) {
    let grid_height = (inventory.slots.len() as f32 / 10.).ceil() as usize;
    egui::Grid::new(entity)
        .min_col_width(32.)
        .max_col_width(32.)
        .spacing([3., 3.])
        .show(ui, |ui| {
            set_hand(ui, hand);
            for row in 0..grid_height {
                for col in 0..10 {
                    let slot_index = row * 10 + col;
                    if let Some(slot) = inventory.slots.get(slot_index) {
                        let item_id = egui::Id::new(entity).with(slot_index);
                        let response = drop_target(ui, item_id, |ui| {
                            if let Some(stack) = slot {
                                let response = drag_source(ui, item_id, |ui| {
                                    item_slot(ui, stack, icons);
                                });

                                response.on_hover_ui_at_pointer(|ui| {
                                    item_tooltip(ui, stack);
                                });
                            }
                        })
                        .response;
                        if response.clicked() {
                            slot_events.send(SlotEvent::clicked(entity, slot_index));
                        };
                    }
                }
                ui.end_row();
            }
        });
}

fn item_tooltip(ui: &mut egui::Ui, stack: &Stack) -> Response {
    egui::Grid::new("item_info")
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
                RichText::new(stack.resource.to_string())
                    .heading()
                    .color(Color32::BLACK),
            );
            ui.end_row();
        })
        .response
}
