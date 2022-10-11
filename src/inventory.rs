use bevy::prelude::*;

use crate::types::{Player, Resource};

pub struct Stack {
    pub resource: Resource,
    pub amount: usize,
}

#[derive(Component)]
pub struct Inventory {
    pub slots: Vec<Option<Stack>>,
}

impl Inventory {
    pub fn new(size: usize) -> Self {
        Self {
            slots: (0..size).map(|_| None).collect(),
        }
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

pub struct InventoryPlugin;
impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(inventory_ui);
    }
}
