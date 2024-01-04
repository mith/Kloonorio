use bevy::app::{App, Plugin};

pub mod inserter_builder;
pub mod miner_builder;
pub mod transport_belt_builder;

pub struct BuilderPlugin;

impl Plugin for BuilderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            inserter_builder::InserterBuilderPlugin,
            miner_builder::MinerBuilderPlugin,
            transport_belt_builder::TransportBeltBuilderPlugin,
        ));
    }
}
