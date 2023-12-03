use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        system::{Commands, Query, Res},
    },
    input::{mouse::MouseButton, Input},
    math::{Vec2, Vec3Swizzles},
};
use bevy_rapier2d::{pipeline::QueryFilter, plugin::RapierContext};
use tracing::{info, instrument};

use crate::{placeable::Building, player::Player, terrain::CursorWorldPos};

pub struct PickerPlugin;

impl Plugin for PickerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, pick_building);
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SelectedBuilding(pub Entity);

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
