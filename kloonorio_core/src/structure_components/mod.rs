pub mod assembler;
pub mod burner;
pub mod inserter;
pub mod miner;
pub mod smelter;
pub mod transport_belt;

use bevy::{
    app::{App, FixedUpdate, Plugin},
    reflect::{Reflect, TypeUuid},
    utils::HashSet,
};
use serde::Deserialize;

use self::{
    assembler::AssemblerPlugin,
    burner::{burner_load, burner_tick},
    inserter::InserterPlugin,
    miner::MinerPlugin,
    smelter::smelter_tick,
    transport_belt::TransportBeltPlugin,
};

pub struct StructureComponentsPlugin;

impl Plugin for StructureComponentsPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<StructureComponent>()
            .add_plugins((
                InserterPlugin,
                TransportBeltPlugin,
                AssemblerPlugin,
                MinerPlugin,
            ))
            .add_systems(FixedUpdate, (smelter_tick, burner_tick, burner_load));
    }
}

#[derive(Clone, Debug, Deserialize, TypeUuid, Reflect)]
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
