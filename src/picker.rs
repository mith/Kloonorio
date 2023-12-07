use bevy::{
    app::{App, Plugin, Update},
    core::Name,
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        system::{Commands, Query, Res},
    },
    input::{mouse::MouseButton, Input},
    math::{Vec2, Vec3Swizzles},
};
use bevy_egui::EguiContexts;
use bevy_rapier2d::{pipeline::QueryFilter, plugin::RapierContext};
use egui::Align2;
use tracing::{info, instrument};

use crate::{placeable::Building, player::Player, terrain::CursorWorldPos};

pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (pick_building, hover_pickable));
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SelectedBuilding(pub Entity);

#[derive(Component)]
pub struct Pickable;

#[instrument(skip(commands, rapier_context, building_query, player_query))]
fn pick_building(
    mut commands: Commands,
    rapier_context: Res<RapierContext>,
    mouse_input: Res<Input<MouseButton>>,
    building_query: Query<&Building>,
    player_query: Query<Entity, With<Player>>,
    cursor_pos: Res<CursorWorldPos>,
) {
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }

    let cursor: Vec2 = cursor_pos.0.xy();
    rapier_context.intersections_with_point(cursor, QueryFilter::new(), |entity| {
        if let Ok(_building) = building_query.get(entity) {
            let player = player_query.single();
            commands.entity(player).insert(SelectedBuilding(entity));
            info!("Selected building: {:?}", entity);
            return false;
        }
        true
    });
}

/// Show a tooltip when hovering over a pickable entity.
fn hover_pickable(
    mut egui_context: EguiContexts,
    rapier_context: Res<RapierContext>,
    pickable_query: Query<&Pickable>,
    name_query: Query<&Name>,
    cursor_pos: Res<CursorWorldPos>,
) {
    let cursor: Vec2 = cursor_pos.0.xy();
    rapier_context.intersections_with_point(cursor, QueryFilter::new(), |entity| {
        let Ok(_pickable) = pickable_query.get(entity) else {
            return true;
        };
        egui::Window::new("Tooltip")
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::RIGHT_BOTTOM, (-5., -5.))
            .title_bar(false)
            .show(egui_context.ctx_mut(), |ui| {
                if let Ok(name) = name_query.get(entity) {
                    ui.label(name.to_string());
                }
            });
        true
    });
}
