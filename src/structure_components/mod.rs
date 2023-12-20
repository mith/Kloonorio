pub mod assembler;
pub mod burner;
pub mod inserter;
pub mod miner;
pub mod smelter;
pub mod transport_belt;

use bevy::{
    app::{App, FixedUpdate, Plugin},
    reflect::TypeUuid,
    utils::HashSet,
};
use serde::Deserialize;

use self::{
    assembler::AssemblerPlugin,
    burner::{burner_load, burner_tick},
    inserter::InserterPlugin,
    miner::miner_tick,
    smelter::smelter_tick,
    transport_belt::TransportBeltPlugin,
};

pub struct StructureComponentsPlugin;

impl Plugin for StructureComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((InserterPlugin, TransportBeltPlugin, AssemblerPlugin))
            .add_systems(
                FixedUpdate,
                (smelter_tick, burner_tick, burner_load, miner_tick),
            );
    }
}

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "990c9ea7-3c00-4d6b-b9f0-c62b86bb9973"]
pub enum StructureComponent {
    Smelter,
    Burner,
    CraftingQueue,
    Inventory(u32),
    Source(u32, HashSet<String>),
    Output(u32),
    Fuel(u32),
    Miner(f32),
    Inserter(f32, u32),
    TransportBelt,
    Assembler,
}
