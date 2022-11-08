use bevy::prelude::*;
use bevy_egui::EguiContext;
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};
use iyes_loopless::prelude::ConditionSet;
use std::collections::VecDeque;

use crate::{
    terrain::{HoveredTile, TerrainStage, TILE_SIZE},
    types::{Player, Resource},
    Recipe,
};

const MAX_STACK_SIZE: u32 = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Stack {
    pub resource: Resource,
    pub amount: u32,
}

impl Stack {
    pub fn new(resource: Resource, amount: u32) -> Self {
        Self { resource, amount }
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
            for slot in self.slots.iter_mut() {
                if let Some(stack) = slot {
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
                } else {
                    *slot = Some(Stack {
                        resource: *resource,
                        amount: amount.min(MAX_STACK_SIZE),
                    });
                    amount = 0;
                }
                if amount == 0 {
                    break;
                }
            }

            if amount > 0 {
                remainder.push((*resource, amount));
            }
        }

        remainder
    }

    pub fn add_item(&mut self, resource: Resource, amount: u32) {
        let mut amount = amount;
        for slot in self.slots.iter_mut() {
            if amount == 0 {
                break;
            }
            if let Some(stack) = slot {
                if stack.resource == resource {
                    let space = MAX_STACK_SIZE - stack.amount;
                    if space > 0 {
                        if amount > space {
                            stack.amount += space;
                            amount -= space;
                        } else {
                            stack.amount += amount;
                            return;
                        }
                    }
                }
            } else {
                *slot = Some(Stack {
                    resource,
                    amount: std::cmp::min(amount, MAX_STACK_SIZE),
                });
                amount -= std::cmp::min(amount, MAX_STACK_SIZE);
            }
        }
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

pub fn inventory_ui(
    mut commands: Commands,
    keyboard_input: Res<Input<KeyCode>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    inventory_query: Query<&Inventory, With<Player>>,
) {
    if keyboard_input.just_pressed(KeyCode::Period) {
        commands
            .spawn_bundle(NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    position: UiRect {
                        left: Val::Px(210.0),
                        bottom: Val::Px(10.0),
                        ..Default::default()
                    },
                    border: UiRect::all(Val::Px(10.0)),
                    size: Size::new(Val::Px(200.0), Val::Px(200.0)),
                    ..Default::default()
                },
                color: UiColor::from(Color::rgb(0.15, 0.15, 0.15)),
                ..Default::default()
            })
            .with_children(|parent| {
                for inventory in inventory_query.iter() {
                    for slot in inventory.slots.iter() {
                        parent.spawn_bundle(NodeBundle {
                            style: Style {
                                position_type: PositionType::Relative,
                                position: UiRect {
                                    left: Val::Px(0.0),
                                    top: Val::Px(0.0),
                                    ..Default::default()
                                },
                                border: UiRect::all(Val::Px(1.0)),
                                size: Size::new(Val::Px(24.0), Val::Px(24.0)),
                                ..Default::default()
                            },
                            color: UiColor::from(Color::rgb(0.2, 0.2, 0.2)),
                            ..Default::default()
                        });
                    }
                }
            });
    }
}

fn is_placeable(resource: Resource) -> bool {
    match resource {
        Resource::StoneFurnace => true,
        _ => false,
    }
}

fn building_size(resource: Resource) -> IVec2 {
    match resource {
        Resource::StoneFurnace => IVec2::new(2, 2),
        _ => IVec2::new(1, 1),
    }
}

#[derive(Component)]
struct Placeable {
    resource: Resource,
    size: IVec2,
}

fn inventory_dev_ui(
    mut commands: Commands,
    mut egui_context: ResMut<EguiContext>,
    inventory_query: Query<&Inventory, With<Player>>,
    player_query: Query<Entity, With<Player>>,
) {
    egui::Window::new("Inventory")
        .default_width(200.0)
        .show(egui_context.ctx_mut(), |ui| {
            for inventory in inventory_query.iter() {
                for stack in inventory.slots.iter().flatten() {
                    ui.horizontal(|ui| {
                        ui.label(format!("{:?}", stack.resource));
                        ui.label(format!("{}", stack.amount));
                        if is_placeable(stack.resource) && ui.button("Place").clicked() {
                            if let Ok(player) = player_query.get_single() {
                                commands.entity(player).insert(Placeable {
                                    resource: stack.resource,
                                    size: building_size(stack.resource),
                                });
                            }
                        }
                    });
                }
            }
        });
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
pub(crate) struct Fueled {
    pub fuel_inventory: Inventory,
    pub fuel_timer: Option<Timer>,
}

impl Fueled {
    fn new() -> Self {
        Self {
            fuel_inventory: Inventory::new(1),
            fuel_timer: None,
        }
    }
}

#[derive(Component)]
pub(crate) struct Smelter {
    pub output: Inventory,
}

impl Smelter {
    pub fn new() -> Self {
        Self {
            output: Inventory::new(1),
        }
    }
}

fn placeable_resource_texture(resource: Resource) -> Option<String> {
    match resource {
        Resource::StoneFurnace => Some("textures/stone_furnace.png".to_string()),
        _ => None,
    }
}

fn placeable(
    mut commands: Commands,
    mut placeable_query: Query<(Entity, &mut Inventory, &Placeable, &HoveredTile)>,
    mouse_input: Res<Input<MouseButton>>,
    ghosts: Query<Entity, With<Ghost>>,
    asset_server: Res<AssetServer>,
    rapier_context: Res<RapierContext>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    // delete old ghosts
    for ghost in ghosts.iter() {
        commands.entity(ghost).despawn_recursive();
    }

    for (player, mut inventory, placeable, hovered_tile) in &mut placeable_query {
        let texture = placeable_resource_texture(placeable.resource)
            .expect("Placeable resource has no texture");
        let texture_handle = asset_server.load(&texture);
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
            info!("Placing {:?}", placeable.resource);
            commands.entity(player).remove::<Placeable>();
            if inventory.remove_items(&[(placeable.resource, 1)]) {
                commands
                    .spawn()
                    .insert_bundle(SpriteSheetBundle {
                        texture_atlas: texture_atlas_handle,
                        ..default()
                    })
                    .insert(Collider::cuboid(11., 11.))
                    .insert_bundle(TransformBundle::from_transform(transform))
                    .insert(Smelter::new())
                    .insert(Fueled::new())
                    .insert(Inventory::new(1))
                    .insert(CraftingQueue::default())
                    .insert(Building)
                    .insert(Name::new("Stone Furnace"));
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
                .label(InventoryStage)
                .after(TerrainStage)
                .with_system(inventory_ui)
                .with_system(inventory_dev_ui)
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
        inventory.add_items(&[(Resource::StoneFurnace, 1)]);
        inventory.add_items(&[(Resource::StoneFurnace, 1)]);
        assert_eq!(
            inventory.slots[0],
            Some(Stack::new(Resource::StoneFurnace, 2))
        );
    }
}
