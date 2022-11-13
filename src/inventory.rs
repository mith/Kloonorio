use bevy::{prelude::*, utils::HashMap};
use bevy_egui::EguiContext;
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};

use egui::{epaint, CursorIcon, InnerResponse, Order, Pos2, Response, Sense, Stroke};
use iyes_loopless::prelude::ConditionSet;
use std::collections::VecDeque;

use crate::{
    burner::Burner,
    smelter::Smelter,
    structure_loader::{Structure, StructureComponent},
    terrain::{HoveredTile, TerrainStage, TILE_SIZE},
    types::{AppState, Player, Resource},
    Recipe,
};

const MAX_STACK_SIZE: u32 = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Stack {
    pub resource: Resource,
    pub amount: u32,
}

impl Stack {
    pub fn new(resource: Resource, amount: u32) -> Self {
        Self { resource, amount }
    }

    pub fn add(&mut self, amount: u32) -> u32 {
        if self.amount + amount > MAX_STACK_SIZE {
            let overflow = self.amount + amount - MAX_STACK_SIZE;
            self.amount = MAX_STACK_SIZE;
            overflow
        } else {
            self.amount += amount;
            0
        }
    }
}

#[derive(Component)]
pub struct Inventory {
    pub slots: Vec<Option<Stack>>,
}

impl Inventory {
    pub fn new(size: u32) -> Self {
        Self {
            slots: (0..size).map(|_| None).collect(),
        }
    }

    pub fn empty(&self) -> bool {
        self.slots.iter().all(|slot| slot.is_none())
    }

    pub fn take_all(&mut self) -> Vec<(Resource, u32)> {
        let mut stacks = Vec::new();
        for slot in self.slots.iter_mut() {
            if let Some(stack) = slot.take() {
                stacks.push((stack.resource, stack.amount));
            }
        }
        stacks
    }

    /// Return true if the inventory has enough space for the items
    pub fn can_add(&self, items: &[(Resource, u32)]) -> bool {
        let mut slots = self.slots.clone();
        let items = items.to_vec();
        for (item_resource, mut item_amount) in items {
            let mut added = false;
            for slot in slots.iter_mut() {
                if let Some(stack) = slot {
                    if stack.resource == item_resource {
                        if stack.amount + item_amount <= MAX_STACK_SIZE {
                            stack.amount += item_amount;
                            added = true;
                            break;
                        } else {
                            let diff = MAX_STACK_SIZE - stack.amount;
                            stack.amount = MAX_STACK_SIZE;
                            item_amount -= diff;
                        }
                    }
                } else {
                    *slot = Some(Stack::new(item_resource, item_amount));
                    added = true;
                    break;
                }
            }
            if !added {
                return false;
            }
        }
        true
    }

    /// Add the items to the inventory, returning the remainder
    pub fn add_items(&mut self, items: &[(Resource, u32)]) -> Vec<(Resource, u32)> {
        let mut remainder = Vec::new();
        for (resource, amount) in items {
            let mut amount = *amount;

            // First iterate over existing stacks
            for stack in self.slots.iter_mut().flatten() {
                if stack.resource == *resource {
                    let space = MAX_STACK_SIZE - stack.amount;
                    if space >= amount {
                        stack.amount += amount;
                        amount = 0;
                    } else {
                        stack.amount = MAX_STACK_SIZE;
                        amount -= space;
                    }
                }
                if amount == 0 {
                    break;
                }
            }

            if amount == 0 {
                return remainder;
            }

            // Then put in the first empty slot
            if let Some(slot) = self.slots.iter_mut().find(|s| s.is_none()) {
                *slot = Some(Stack {
                    resource: resource.clone(),
                    amount: amount.min(MAX_STACK_SIZE),
                });
                amount = 0;
            }

            if amount > 0 {
                remainder.push((resource.clone(), amount));
            }
        }

        remainder
    }

    pub fn add_item(&mut self, resource: Resource, amount: u32) -> Vec<(Resource, u32)> {
        self.add_items(&[(resource, amount)])
    }

    pub fn has_items(&self, items: &[(Resource, u32)]) -> bool {
        for (resource, amount) in items {
            let mut amount = *amount;
            for slot in self.slots.iter() {
                if amount == 0 {
                    break;
                }
                if let Some(stack) = slot {
                    if stack.resource == *resource {
                        if stack.amount >= amount {
                            amount = 0;
                        } else {
                            amount -= stack.amount;
                        }
                    }
                }
            }
            if amount > 0 {
                return false;
            }
        }
        true
    }

    /// Removes all items atomically, returning true on success
    pub fn remove_items(&mut self, items: &[(Resource, u32)]) -> bool {
        if !self.has_items(items) {
            return false;
        }

        for (resource, amount) in items {
            let mut amount = *amount;
            for slot in self.slots.iter_mut() {
                if amount == 0 {
                    break;
                }
                if let Some(stack) = slot {
                    if stack.resource == *resource {
                        if stack.amount >= amount {
                            stack.amount -= amount;
                            amount = 0;
                        } else {
                            amount -= stack.amount;
                            stack.amount = 0;
                        }
                        if stack.amount == 0 {
                            *slot = None;
                        }
                    }
                }
            }
        }
        true
    }
}

#[derive(Component)]
struct Placeable {
    structure: String,
    size: IVec2,
}

fn drag_source(ui: &mut egui::Ui, id: egui::Id, body: impl FnOnce(&mut egui::Ui)) {
    let is_being_dragged = ui.memory().is_being_dragged(id);

    if !is_being_dragged {
        let response = ui.scope(body).response;

        // Check for drags:
        let response = ui.interact(response.rect, id, Sense::drag());
        if response.hovered() {
            ui.output().cursor_icon = CursorIcon::Grab;
        }
    } else {
        ui.output().cursor_icon = CursorIcon::Grabbing;

        // Paint the body to a new layer:
        let layer_id = egui::LayerId::new(Order::Tooltip, id);
        let response = ui.with_layer_id(layer_id, body).response;

        if let Some(pointer_pos) = ui.ctx().pointer_interact_pos() {
            let delta = pointer_pos - response.rect.center() + egui::Vec2::new(10., 10.);
            ui.ctx().translate_layer(layer_id, delta);
        }
    }
}

fn drop_target<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> InnerResponse<R> {
    let is_being_dragged = ui.memory().is_anything_being_dragged();
    let outer_rect_bounds = ui.available_rect_before_wrap();
    let where_to_put_background = ui.painter().add(egui::Shape::Noop);
    let mut content_ui = ui.child_ui(outer_rect_bounds, *ui.layout());
    let ret = body(&mut content_ui);
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(32., 32.), Sense::hover());
    let (style, bg_fill) = if is_being_dragged && response.hovered() {
        (ui.visuals().widgets.active, egui::Color32::RED)
    } else {
        (ui.visuals().widgets.inactive, egui::Color32::from_gray(45))
    };
    if response.dragged() {
        ui.ctx().output().cursor_icon = CursorIcon::Grab;
    }
    ui.painter().set(
        where_to_put_background,
        epaint::RectShape {
            rounding: style.rounding,
            fill: bg_fill,
            stroke: Stroke::none(),
            rect,
        },
    );
    InnerResponse::new(ret, response)
}

pub fn resource_stack(
    ui: &mut egui::Ui,
    stack: &Stack,
    icons: &HashMap<String, egui::TextureId>,
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

fn inventory_ui(
    mut commands: Commands,
    mut egui_context: ResMut<EguiContext>,
    mut inventory_query: Query<&mut Inventory, With<Player>>,
    player_query: Query<Entity, With<Player>>,
    structures: Res<HashMap<String, Structure>>,
    icons: Res<HashMap<String, egui::TextureId>>,
) {
    egui::Window::new("Inventory")
        .resizable(false)
        .show(egui_context.ctx_mut(), |ui| {
            for mut inventory in &mut inventory_query {
                let (source_slot, drop_slot) =
                    inventory_grid("character", &mut inventory, ui, &icons);
                if let (Some(source_slot), Some(drop_slot)) = (source_slot, drop_slot) {
                    if ui.input().pointer.any_released() {
                        drop_within_inventory(
                            &mut inventory,
                            (source_slot) as usize,
                            (drop_slot) as usize,
                        );
                    }
                } else if let Some(source_slot) = source_slot {
                    if !ui.ui_contains_pointer() {
                        if let Some(stack) = inventory.slots[source_slot].clone() {
                            if let Resource::Structure(structure_name) = stack.resource {
                                if let Ok(player) = player_query.get_single() {
                                    let structure = structures.get(&structure_name).unwrap();
                                    commands.entity(player).insert(Placeable {
                                        structure: structure_name.clone(),
                                        size: structure.size,
                                    });
                                }
                            }
                        }
                    } else {
                        commands
                            .entity(player_query.get_single().unwrap())
                            .remove::<Placeable>();
                    }
                }
            }
        });
}

pub type SlotIndex = usize;
pub type Drag = (Option<SlotIndex>, Option<SlotIndex>);

pub fn drop_between_inventories(inventories: &mut [(&mut Inventory, Drag)]) {
    for (inventory, drag) in inventories.iter_mut() {
        // If the item is dropped in the same inventory
        if let (Some(source_slot), Some(target_slot)) = drag {
            drop_within_inventory(*inventory, *source_slot, *target_slot)
        }
    }

    let dragged: (
        Option<(&mut Inventory, SlotIndex)>,
        Option<(&mut Inventory, SlotIndex)>,
    ) = inventories
        .iter_mut()
        .fold((None, None), |acc, inv| match inv {
            (ref mut inventory, (Some(source_slot), None)) => {
                (Some((inventory, *source_slot)), acc.1)
            }
            (ref mut inventory, (None, Some(target_slot))) => {
                (acc.0, (Some((inventory, *target_slot))))
            }
            _ => acc,
        });

    if let (Some((source_inventory, source_slot)), Some((target_inventory, target_slot))) = dragged
    {
        if let Some(mut source_stack) = source_inventory.slots.get(source_slot).unwrap().clone() {
            if let Some(target_stack) = target_inventory.slots.get(target_slot).unwrap() {
                let mut target_stack = target_stack.clone();
                if target_stack.resource == source_stack.resource {
                    info!("Adding source stack to target stack");
                    let remainder = target_stack.add(source_stack.amount);
                    source_stack.amount = remainder;
                    target_inventory.slots[target_slot] = Some(target_stack);
                    source_inventory.slots[source_slot] = Some(source_stack);
                } else {
                    info!("Swapping stacks");
                    target_inventory.slots[target_slot] = Some(source_stack);
                    source_inventory.slots[source_slot] = Some(target_stack);
                }
            } else {
                info!("Moving source stack to target slot");
                target_inventory.slots[target_slot] = Some(source_stack);
                source_inventory.slots[source_slot] = None;
            }
        }
    }
}

pub fn drop_within_inventory(inventory: &mut Inventory, source_slot: usize, target_slot: usize) {
    if let Some(mut source_stack) = inventory.slots.get(source_slot).unwrap().clone() {
        if let Some(target_stack) = inventory.slots.get(target_slot).unwrap() {
            let mut target_stack = target_stack.clone();
            if target_stack.resource == source_stack.resource {
                info!("Adding source stack to target stack");
                let remainder = target_stack.add(source_stack.amount);
                source_stack.amount = remainder;
                inventory.slots[target_slot] = Some(target_stack);
                inventory.slots[source_slot] = Some(source_stack);
            } else {
                info!("Swapping stacks");
                inventory.slots[target_slot] = Some(source_stack);
                inventory.slots[source_slot] = Some(target_stack);
            }
        } else {
            info!("Moving source stack to target slot");
            inventory.slots[target_slot] = Some(source_stack);
            inventory.slots[source_slot] = None;
        }
    }
}

pub fn inventory_grid(
    name: &str,
    inventory: &mut Inventory,
    ui: &mut egui::Ui,
    icons: &HashMap<String, egui::TextureId>,
) -> (Option<usize>, Option<usize>) {
    let mut source_slot = None;
    let mut drop_slot = None;
    let grid_height = (inventory.slots.len() as f32 / 10.).ceil() as usize;
    egui::Grid::new(name)
        .min_col_width(32.)
        .max_col_width(32.)
        .spacing([3., 3.])
        .show(ui, |ui| {
            for row in 0..grid_height {
                for col in 0..10 {
                    if let Some(slot) = inventory.slots.get(row * 10 + col) {
                        let item_id = egui::Id::new(name).with(col).with(row);
                        let response = drop_target(ui, |ui| {
                            if let Some(stack) = slot {
                                drag_source(ui, item_id, |ui| {
                                    let response = resource_stack(ui, stack, icons);
                                    if !ui.memory().is_being_dragged(item_id) {
                                        response.on_hover_text_at_pointer(stack.resource.name());
                                    } else {
                                        source_slot = Some(row * 10 + col);
                                    }
                                });
                            }
                        });
                        if response.response.hovered() {
                            drop_slot = Some(row * 10 + col);
                        }
                    }
                }
                ui.end_row();
            }
        });
    (source_slot, drop_slot)
}

#[derive(Component, Default)]
pub struct CraftingQueue(pub VecDeque<ActiveCraft>);

pub struct ActiveCraft {
    pub blueprint: Recipe,
    pub timer: Timer,
}

#[derive(Component)]
struct Ghost;

#[derive(Component)]
pub(crate) struct Building;

#[derive(Component)]
pub struct Source(pub Inventory);

#[derive(Component)]
pub struct Output(pub Inventory);

fn placeable(
    mut commands: Commands,
    mut placeable_query: Query<(Entity, &mut Inventory, &Placeable, &HoveredTile)>,
    mouse_input: Res<Input<MouseButton>>,
    ghosts: Query<Entity, With<Ghost>>,
    asset_server: Res<AssetServer>,
    rapier_context: Res<RapierContext>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    structures: Res<HashMap<String, Structure>>,
) {
    // delete old ghosts
    for ghost in ghosts.iter() {
        commands.entity(ghost).despawn_recursive();
    }

    for (player, mut inventory, placeable, hovered_tile) in &mut placeable_query {
        let structure = structures.get(&placeable.structure).unwrap();
        let texture_handle = asset_server.load(&format!("textures/{}.png", &structure.texture));
        let texture_atlas = TextureAtlas::from_grid(
            texture_handle,
            placeable.size.as_vec2() * Vec2::new(TILE_SIZE.x, TILE_SIZE.y),
            2,
            1,
        );
        let texture_atlas_handle = texture_atlases.add(texture_atlas);
        let translation =
            hovered_tile.tile_center + Vec2::new(0.5 * TILE_SIZE.x, 0.5 * TILE_SIZE.y);
        let transform = Transform::from_translation(translation.extend(1.0));

        if rapier_context
            .intersection_with_shape(
                translation,
                0.,
                &Collider::cuboid(16., 16.),
                QueryFilter::new(),
            )
            .is_some()
        {
            commands
                .spawn()
                .insert_bundle(SpriteSheetBundle {
                    transform,
                    texture_atlas: texture_atlas_handle,
                    sprite: TextureAtlasSprite {
                        color: Color::rgba(1.0, 0.3, 0.3, 0.5),
                        ..default()
                    },
                    ..default()
                })
                .insert(Ghost);
        } else if mouse_input.just_pressed(MouseButton::Left) {
            info!("Placing {:?}", placeable.structure);
            commands.entity(player).remove::<Placeable>();
            if inventory.remove_items(&[(Resource::Structure(structure.name.clone()), 1)]) {
                let structure_entity = commands
                    .spawn()
                    .insert_bundle(SpriteSheetBundle {
                        texture_atlas: texture_atlas_handle,
                        ..default()
                    })
                    .insert(Collider::cuboid(11., 11.))
                    .insert_bundle(TransformBundle::from_transform(transform))
                    .insert(Building)
                    .insert(Name::new(structure.name.to_string()))
                    .id();

                for component in &structure.components {
                    match component {
                        StructureComponent::Smelter => {
                            commands.entity(structure_entity).insert(Smelter);
                        }
                        StructureComponent::Burner => {
                            commands.entity(structure_entity).insert(Burner::new());
                        }
                        StructureComponent::CraftingQueue => {
                            commands
                                .entity(structure_entity)
                                .insert(CraftingQueue::default());
                        }
                        StructureComponent::Inventory(slots) => {
                            commands
                                .entity(structure_entity)
                                .insert(Inventory::new(*slots));
                        }
                        StructureComponent::Source(slots) => {
                            commands
                                .entity(structure_entity)
                                .insert(Source(Inventory::new(*slots)));
                        }
                        StructureComponent::Output(slots) => {
                            commands
                                .entity(structure_entity)
                                .insert(Output(Inventory::new(*slots)));
                        }
                    }
                }
            }
        } else {
            commands
                .spawn()
                .insert_bundle(SpriteSheetBundle {
                    transform,
                    texture_atlas: texture_atlas_handle,
                    sprite: TextureAtlasSprite {
                        color: Color::rgba(1.0, 1.0, 1.0, 0.5),
                        ..default()
                    },
                    ..default()
                })
                .insert(Ghost);
        }
    }
}

#[derive(SystemLabel)]
pub struct InventoryStage;

pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            ConditionSet::new()
                .run_in_state(AppState::Running)
                .label(InventoryStage)
                .after(TerrainStage)
                .with_system(inventory_ui)
                .with_system(placeable)
                .into(),
        );
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_has_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        assert!(inventory.has_items(&[(Resource::Stone, 5), (Resource::Wood, 10)]));
        assert!(!inventory.has_items(&[(Resource::Stone, 5), (Resource::Wood, 30)]));
    }

    #[test]
    fn test_remove_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        inventory.remove_items(&[(Resource::Stone, 5), (Resource::Wood, 10)]);
        assert_eq!(inventory.slots[0], Some(Stack::new(Resource::Stone, 5)));
        assert_eq!(inventory.slots[1], Some(Stack::new(Resource::Wood, 10)));
    }

    #[test]
    fn test_remove_items_empty() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        inventory.remove_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        assert!(inventory.slots.iter().all(|s| s.is_none()));
    }

    #[test]
    fn test_remove_items_not_enough() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        assert!(!inventory.remove_items(&[(Resource::Stone, 5), (Resource::Wood, 30)]));
        assert_eq!(inventory.slots[0], Some(Stack::new(Resource::Stone, 10)));
        assert_eq!(inventory.slots[1], Some(Stack::new(Resource::Wood, 20)));
    }
    #[test]
    fn test_add_items() {
        let mut inventory = Inventory::new(12);
        inventory.add_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        assert_eq!(inventory.slots[0], Some(Stack::new(Resource::Stone, 10)));
        assert_eq!(inventory.slots[1], Some(Stack::new(Resource::Wood, 20)));
    }

    #[test]
    fn test_add_items_remainder() {
        let mut inventory = Inventory::new(1);
        let remainder = inventory.add_items(&[(Resource::Stone, 10), (Resource::Wood, 20)]);
        assert_eq!(inventory.slots[0], Some(Stack::new(Resource::Stone, 10)));
        assert_eq!(remainder, vec![(Resource::Wood, 20)]);
    }

    #[test]
    fn test_add_items_stack() {
        let mut inventory = Inventory::new(2);
        inventory.slots[1] = Some(Stack::new(Resource::Structure("Stone furnace".into()), 10));
        inventory.add_items(&[(Resource::Structure("Stone furnace".into()), 1)]);
        assert_eq!(
            inventory.slots[1],
            Some(Stack::new(Resource::Structure("Stone furnace".into()), 11))
        );
    }
}
