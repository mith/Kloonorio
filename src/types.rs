use std::{borrow::Cow, collections::VecDeque, ops::Deref};

use bevy::{asset::LoadedFolder, prelude::*, reflect::TypeUuid};
use serde::Deserialize;

use crate::{
    item_loader::ItemAsset, recipe_loader::RecipesAsset, structure_loader::StructuresAsset,
};

#[derive(Clone, PartialEq, Eq, Component, Debug, Hash, States, Default, Reflect)]
pub enum AppState {
    #[default]
    Loading,
    Running,
}

// TODO: move this to loading.rs
#[derive(Default, Resource, Reflect)]
pub struct GameState {
    pub map_loaded: bool,
    pub spawned: bool,
    pub recipes_handle: Handle<RecipesAsset>,
    pub recipes_loaded: bool,
    pub structures_handle: Handle<StructuresAsset>,
    pub structures_loaded: bool,
    pub icons_loaded: bool,
    pub icons_handle: Handle<LoadedFolder>,
    pub items_loaded: bool,
    pub resources_handle: Handle<ItemAsset>,
}

#[derive(Component)]
pub struct StaticDimensions(pub IVec2);

/// An item is a "thing" that can be stored in an inventory, used in or produced by a recipe, etc.
#[derive(Hash, Eq, PartialEq, Debug, Clone, TypeUuid, Reflect, Deserialize)]
#[serde(from = "String")]
#[uuid = "28a860c7-96ee-44e5-ae3b-8a25d9a863d5"]
pub struct Item(Name);

impl Item {
    pub fn new(name: impl Into<Cow<'static, str>>) -> Self {
        Self(Name::new(name))
    }
}

impl std::fmt::Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for Item {
    fn from(name: String) -> Self {
        Self(Name::new(name))
    }
}

impl AsRef<str> for Item {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Deref for Item {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Powered;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Working;

#[derive(Component, Default, Reflect)]
pub struct CraftingQueue(pub VecDeque<ActiveCraft>);

#[derive(Reflect)]
pub struct ActiveCraft {
    pub recipe: Recipe,
    pub timer: Timer,
}

#[derive(Clone, Debug, Deserialize, TypeUuid, Reflect)]
#[uuid = "1ca725c1-5a0d-484f-8d04-a5a42960e208"]
pub struct Recipe {
    pub ingredients: Vec<(Item, u32)>,
    pub products: Vec<(Item, u32)>,
    pub crafting_time: f32,
    pub name: String,
}

#[derive(Component)]
pub struct Pickup;

#[derive(Component)]
pub struct Dropoff;
