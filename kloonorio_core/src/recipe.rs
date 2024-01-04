use std::ops::{Deref, DerefMut};

use bevy::{
    ecs::system::Resource,
    reflect::{Reflect, TypeUuid},
    utils::HashMap,
};
use serde::Deserialize;

use crate::item::Item;

#[derive(Clone, Debug, Deserialize, TypeUuid, Reflect)]
#[uuid = "1ca725c1-5a0d-484f-8d04-a5a42960e208"]
pub struct Recipe {
    pub ingredients: Vec<(Item, u32)>,
    pub products: Vec<(Item, u32)>,
    pub crafting_time: f32,
    pub name: String,
}

#[derive(Resource, Default, Reflect)]
pub struct Recipes(HashMap<String, Recipe>);

impl Deref for Recipes {
    type Target = HashMap<String, Recipe>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Recipes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
