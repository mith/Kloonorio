use bevy::{
    app::{App, Plugin, Update},
    ecs::schedule::{common_conditions::in_state, IntoSystemConfigs},
};
use kloonorio_core::types::AppState;

pub mod inserter_builder;
pub mod miner_builder;
pub mod placeable;
pub mod transport_belt_builder;

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            inserter_builder::InserterBuilderPlugin,
            miner_builder::MinerBuilderPlugin,
            transport_belt_builder::TransportBeltBuilderPlugin,
        ))
        .add_systems(
            Update,
            (placeable::placeable, placeable::placeable_rotation)
                .run_if(in_state(AppState::Running)),
        );
    }
}
