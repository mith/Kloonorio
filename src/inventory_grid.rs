use std::ops::Deref;

use bevy::prelude::*;
use bevy::utils::HashMap;
use egui::{epaint, Color32, CursorIcon, InnerResponse, Order, Pos2, Response, Sense, Stroke};

use crate::inventory::{Inventory, Stack};

pub const HIGHLIGHT_COLOR: Color32 = egui::Color32::from_rgb(252, 161, 3);

fn item_in_hand(ui: &mut egui::Ui) -> Option<InventoryIndex> {
    let hand_id = egui::Id::new("hand");
    ui.memory().data.get_temp::<InventoryIndex>(hand_id)
}

fn set_item_in_hand(ui: &mut egui::Ui, item: Option<InventoryIndex>) {
    let hand_id = egui::Id::new("hand");
    if let Some(item) = item {
        ui.memory()
            .data
            .insert_temp::<InventoryIndex>(hand_id, item);
    } else {
        ui.memory().data.remove::<InventoryIndex>(hand_id);
    }
}

fn drag_source(ui: &mut egui::Ui, id: egui::Id, body: impl FnOnce(&mut egui::Ui)) -> Response {
    let is_being_dragged = item_in_hand(ui).filter(|h| h.item_id == id).is_some();

    if !is_being_dragged {
        let response = ui.scope(body).response;

        // Check for drags:
        let response = ui.interact(
            response.rect,
            id,
            Sense::click_and_drag().union(Sense::hover()),
        );
        if response.hovered() {
            ui.output().cursor_icon = CursorIcon::Grab;
        }
        return response;
    } else {
        ui.output().cursor_icon = CursorIcon::Grabbing;

        // Paint the body to a new layer:
        let layer_id = egui::LayerId::new(Order::Tooltip, id);
        let response = ui.with_layer_id(layer_id, body).response;

        if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
            let delta = pointer_pos - response.rect.center() + egui::Vec2::new(10., 10.);
            ui.ctx().translate_layer(layer_id, delta);
        }
        return response;
    }
}

fn drop_target<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    body: impl FnOnce(&mut egui::Ui) -> R,
) -> InnerResponse<R> {
    let being_dragged = ui
        .memory()
        .data
        .get_temp::<egui::Id>(egui::Id::new("hand"))
        .map_or(false, |h| h == id);
    let outer_rect_bounds = ui.available_rect_before_wrap();
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(32., 32.), Sense::hover());
    let (style, bg_fill) = if being_dragged || response.hovered() {
        (ui.visuals().widgets.active, HIGHLIGHT_COLOR)
    } else {
        (ui.visuals().widgets.inactive, egui::Color32::from_gray(45))
    };
    if response.dragged() {
        ui.ctx().output().cursor_icon = CursorIcon::Grab;
    }
    ui.painter().add(epaint::RectShape {
        rounding: style.rounding,
        fill: bg_fill,
        stroke: Stroke::none(),
        rect,
    });

    let mut content_ui = ui.child_ui(outer_rect_bounds, *ui.layout());
    let ret = body(&mut content_ui);
    InnerResponse::new(ret, response)
}

pub fn resource_stack(
    ui: &mut egui::Ui,
    stack: &Stack,
    icons: &HashMap<String, egui::TextureId>,
) -> Response {
    let response = resource_icon(ui, stack, icons);

    let font_id = egui::FontId::proportional(16.);
    let layout = ui
        .fonts()
        .layout_no_wrap(stack.amount.to_string(), font_id, egui::Color32::WHITE);
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
    let icon_name = &stack.resource.name().to_lowercase().replace(" ", "_");
    let response = {
        if let Some(egui_img) = icons.get(icon_name) {
            ui.image(*egui_img, [32., 32.])
        } else if let Some(no_icon_img) = icons.get("no_icon") {
            ui.image(*no_icon_img, [32., 32.])
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
    pub item_id: egui::Id,
}

impl InventoryIndex {
    pub fn new(entity: Entity, slot: SlotIndex) -> Self {
        Self {
            entity,
            slot,
            item_id: egui::Id::new(entity).with(slot),
        }
    }
}

#[derive(Component, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hand(pub Option<InventoryIndex>);

#[derive(Component, Debug, Clone, PartialEq, Eq, Hash)]
pub struct HoverSlot(pub InventoryIndex);
impl HoverSlot {
    pub fn new(entity: Entity, slot: SlotIndex, item_id: egui::Id) -> Self {
        HoverSlot(InventoryIndex {
            entity,
            slot,
            item_id,
        })
    }
}

pub enum SlotEvent {
    Clicked(InventoryIndex),
}

impl SlotEvent {
    pub fn clicked(entity: Entity, slot: SlotIndex, item_id: egui::Id) -> Self {
        Self::Clicked(InventoryIndex {
            entity,
            slot,
            item_id,
        })
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
            set_item_in_hand(ui, hand.0.clone());
            for row in 0..grid_height {
                for col in 0..10 {
                    let slot_index = row * 10 + col;
                    if let Some(slot) = inventory.slots.get(slot_index) {
                        let item_id = egui::Id::new(entity).with(slot_index);
                        drop_target(ui, item_id, |ui| {
                            if let Some(stack) = slot {
                                let response = drag_source(ui, item_id, |ui| {
                                    resource_stack(ui, stack, icons);
                                });
                                if response.hovered() {
                                    response
                                        .clone()
                                        .on_hover_text_at_pointer(stack.resource.name());
                                }
                                if response.clicked() || response.dragged() {
                                    slot_events
                                        .send(SlotEvent::clicked(entity, slot_index, item_id));
                                }
                            }
                        });
                    }
                }
                ui.end_row();
            }
        });
}