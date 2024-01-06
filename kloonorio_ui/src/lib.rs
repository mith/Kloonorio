pub mod building_ui;
pub mod character_ui;
mod crafting_queue_ui;
mod debug;
pub mod drag_and_drop;
mod healthbar;
pub mod hotbar;
pub mod icon;
mod interact_ui;
pub mod inventory_grid;
pub mod picker;
mod tooltip;
mod util;

use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        schedule::{common_conditions::in_state, IntoSystemConfigs, SystemSet},
        system::{Commands, Query},
    },
};
use bevy_egui::{EguiContexts, EguiPlugin};
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use picker::PickerPlugin;

use self::{
    character_ui::CharacterUiPlugin,
    debug::DebugPlugin,
    drag_and_drop::{clear_hand, drop_system},
    hotbar::HotbarPlugin,
    inventory_grid::SlotEvent,
};
use kloonorio_core::{player::Player, types::AppState};

pub struct KloonorioUiPlugin;

#[derive(Clone, Debug, PartialEq, Eq, Hash, SystemSet)]
pub struct UiSet;

impl Plugin for KloonorioUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((EguiPlugin, DefaultInspectorConfigPlugin))
            .add_plugins((CharacterUiPlugin, HotbarPlugin, PickerPlugin, DebugPlugin))
            .add_systems(
                Update,
                (
                    hovering_ui,
                    building_ui::building_ui,
                    crafting_queue_ui::crafting_queue_ui,
                    clear_hand,
                    interact_ui::interaction_ui,
                    healthbar::healthbar,
                )
                    .run_if(in_state(AppState::Running)),
            )
            .add_event::<SlotEvent>()
            .add_systems(
                Update,
                drop_system.after(UiSet).run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct HoveringUI;

fn hovering_ui(
    mut commands: Commands,
    mut egui_context: EguiContexts,
    hovering_player_query: Query<Entity, (With<Player>, With<HoveringUI>)>,
    non_hovering_player_query: Query<Entity, (With<Player>, Without<HoveringUI>)>,
) {
    if egui_context.ctx_mut().is_pointer_over_area() {
        for entity in non_hovering_player_query.iter() {
            commands.entity(entity).insert(HoveringUI);
        }
    } else {
        for entity in hovering_player_query.iter() {
            commands.entity(entity).remove::<HoveringUI>();
        }
    }
}
