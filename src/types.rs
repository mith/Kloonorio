use std::{collections::VecDeque, fmt::Display};

use bevy::{asset::LoadedFolder, prelude::*, reflect::TypeUuid};
use serde::Deserialize;

use crate::{
    intermediate_loader::IntermediateAsset, recipe_loader::RecipesAsset,
    structure_loader::StructuresAsset,
};

#[derive(Clone, PartialEq, Eq, Component, Debug, Hash, States, Default)]
pub enum AppState {
    #[default]
    Loading,
    Running,
}

#[derive(Default, Resource)]
pub struct GameState {
    pub map_loaded: bool,
    pub spawned: bool,
    pub recipes_handle: Handle<RecipesAsset>,
    pub recipes_loaded: bool,
    pub structures_handle: Handle<StructuresAsset>,
    pub structures_loaded: bool,
    pub icons_loaded: bool,
    pub icons_handle: Handle<LoadedFolder>,
    pub resources_loaded: bool,
    pub resources_handle: Handle<IntermediateAsset>,
}

#[derive(Component)]
pub struct StaticDimensions(pub IVec2);

#[derive(Hash, Eq, PartialEq, Debug, Clone, Deserialize, TypeUuid, Reflect)]
#[uuid = "28a860c7-96ee-44e5-ae3b-8a25d9a863d5"]
pub enum Product {
    Intermediate(String),
    Structure(String),
}

impl Display for Product {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Product::Intermediate(name) => write!(f, "{}", name),
            Product::Structure(name) => write!(f, "{}", name),
        }
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Powered;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Working;

#[derive(Component, Default)]
pub struct CraftingQueue(pub VecDeque<ActiveCraft>);

pub struct ActiveCraft {
    pub blueprint: Recipe,
    pub timer: Timer,
}

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "1ca725c1-5a0d-484f-8d04-a5a42960e208"]
pub struct Recipe {
    pub materials: Vec<(Product, u32)>,
    pub products: Vec<(Product, u32)>,
    pub crafting_time: f32,
    pub name: String,
}
