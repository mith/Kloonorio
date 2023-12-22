use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        system::{Commands, Query, Res, Resource},
    },
    input::{mouse::MouseButton, Input},
    math::Vec3Swizzles,
    time::{Time, Timer, TimerMode},
    transform::components::Transform,
};
use bevy_ecs_tilemap::tiles::TileTextureIndex;
use bevy_egui::EguiContexts;
use egui::Align2;

use crate::{
    inventory::Inventory,
    player::Player,
    terrain::{CursorWorldPos, HoveredTile, COAL, IRON, STONE, TREE},
    types::Item,
};

pub struct InteractPlugin;

impl Plugin for InteractPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                interact,
                interact_cancel,
                interact_completion,
                interaction_ui,
            ),
        );
    }
}

#[derive(Component)]
struct MineCountdown {
    timer: Timer,
    target: Entity,
}

pub fn is_minable(tile: u32) -> bool {
    matches!(tile, COAL | IRON | STONE | TREE)
}

#[derive(Resource)]
pub struct PlayerSettings {
    pub max_mining_distance: f32,
}

fn interact(
    mut commands: Commands,
    cursor_pos: Res<CursorWorldPos>,
    tile_query: Query<&TileTextureIndex>,
    mouse_button_input: Res<Input<MouseButton>>,
    player_query: Query<(Entity, &Transform, &HoveredTile), (With<Player>, Without<MineCountdown>)>,
    player_settings: Res<PlayerSettings>,
) {
    if player_query.is_empty() {
        return;
    }
    let (player_entity, player_transform, hovered_tile) = player_query.single();

    if !mouse_button_input.pressed(MouseButton::Right) {
        return;
    };

    if let Ok(tile_texture) = tile_query.get(hovered_tile.entity) {
        let tile_distance = player_transform
            .translation
            .xy()
            .distance(cursor_pos.0.xy());
        if is_minable(tile_texture.0) && tile_distance < player_settings.max_mining_distance {
            commands.entity(player_entity).insert(MineCountdown {
                timer: Timer::from_seconds(1.0, TimerMode::Repeating),
                target: hovered_tile.entity,
            });
        }
    }
}

fn interact_cancel(
    mut commands: Commands,
    player_query: Query<Entity, With<MineCountdown>>,
    mouse_button_input: Res<Input<MouseButton>>,
) {
    if mouse_button_input.just_released(MouseButton::Right) {
        for player_entity in &player_query {
            commands.entity(player_entity).remove::<MineCountdown>();
        }
    }
}

fn interact_completion(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Inventory, &mut MineCountdown)>,
    tile_query: Query<&TileTextureIndex>,
) {
    for (entity, mut inventory, mut interaction) in &mut query {
        if interaction.timer.tick(time.delta()).just_finished() {
            commands.entity(entity).remove::<MineCountdown>();
            let tile_entity = interaction.target;
            if let Ok(tile_texture) = tile_query.get(tile_entity) {
                match tile_texture.0 {
                    COAL => inventory.add_item(&Item::new("Coal"), 1),
                    IRON => inventory.add_item(&Item::new("Iron ore"), 1),
                    STONE => inventory.add_item(&Item::new("Stone"), 1),
                    TREE => inventory.add_item(&Item::new("Wood"), 1),
                    _ => 0,
                };
            }
        }
    }
}

fn interaction_ui(mut egui_context: EguiContexts, interact_query: Query<&MineCountdown>) {
    if let Ok(interact) = interact_query.get_single() {
        egui::Window::new("Interaction")
            .anchor(Align2::CENTER_BOTTOM, (0., -10.))
            .title_bar(false)
            .resizable(false)
            .show(egui_context.ctx_mut(), |ui| {
                ui.add(egui::ProgressBar::new(interact.timer.percent()));
            });
    }
}
