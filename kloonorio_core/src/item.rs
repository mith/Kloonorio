use std::{
    borrow::Cow,
    ops::{Deref, DerefMut},
};

use bevy::{
    app::{App, Plugin},
    core::Name,
    ecs::system::Resource,
    reflect::{Reflect, TypeUuid},
    utils::HashMap,
};
use serde::Deserialize;

pub struct ItemPlugin;

impl Plugin for ItemPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Item>();
    }
}

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

#[derive(Resource, Default, Reflect)]
pub struct Items(HashMap<String, Item>);

impl Deref for Items {
    type Target = HashMap<String, Item>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Items {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
