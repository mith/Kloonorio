use std::collections::VecDeque;

use bevy::{prelude::*, reflect::TypeUuid};
use serde::Deserialize;

use crate::{structure_loader::StructuresAsset, RecipeAsset};

#[derive(Clone, PartialEq, Eq, Component, Debug, Hash)]
pub enum AppState {
    Loading,
    Running,
}

#[derive(Default, Resource)]
pub struct GameState {
    pub map_loaded: bool,
    pub spawned: bool,
    pub recipes_handle: Handle<RecipeAsset>,
    pub recipes_loaded: bool,
    pub structures_handle: Handle<StructuresAsset>,
    pub structures_loaded: bool,
    pub icons_loaded: bool,
    pub icons_handle: Vec<HandleUntyped>,
}

#[derive(Component)]
pub struct Player;

#[derive(Hash, Eq, PartialEq, Debug, Clone, Deserialize, TypeUuid)]
#[uuid = "28a860c7-96ee-44e5-ae3b-8a25d9a863d5"]
pub enum Product {
    Coal,
    IronOre,
    Wood,
    Stone,
    IronPlate,
    IronGearWheel,
    Structure(String),
}

impl Product {
    pub fn name(&self) -> String {
        match self {
            Product::Coal => "Coal".to_string(),
            Product::IronOre => "Iron ore".to_string(),
            Product::Wood => "Wood".to_string(),
            Product::Stone => "Stone".to_string(),
            Product::IronPlate => "Iron plate".to_string(),
            Product::IronGearWheel => "Iron gear wheel".to_string(),
            Product::Structure(s) => s.to_string(),
        }
    }
}

#[derive(Component)]
pub struct Powered;

#[derive(Component)]
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, SystemLabel)]
pub struct UiPhase;
