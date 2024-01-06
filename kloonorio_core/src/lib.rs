use bevy::app::{PluginGroup, PluginGroupBuilder};

pub mod discrete_rotation;
pub mod drop;
pub mod health;
pub mod inventory;
pub mod item;
pub mod mineable;
pub mod player;
pub mod recipe;
pub mod structure;
pub mod structure_components;
pub mod tile_occupants;
pub mod types;

pub struct KloonorioCorePlugins;

impl PluginGroup for KloonorioCorePlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(item::ItemPlugin)
            .add(discrete_rotation::DiscreteRotationPlugin)
            .add(structure_components::StructureComponentsPlugin)
            .add(tile_occupants::TileOccupantsPlugin)
            .add(health::HealthPlugin)
    }
}
