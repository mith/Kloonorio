use std::ops::{Deref, DerefMut};

use bevy::{
    ecs::system::Resource,
    math::{IVec2, Vec2},
    reflect::{Reflect, TypeUuid},
    utils::HashMap,
};
use serde::Deserialize;

use crate::structure_components::StructureComponent;

#[derive(Clone, Debug, Deserialize, TypeUuid, Reflect)]
#[uuid = "540f864d-3e80-4e5d-8be5-1846d7be2484"]
pub struct Structure {
    pub name: String,
    pub size: IVec2,
    pub collider: Vec2,
    pub sides: u32,
    pub components: Vec<StructureComponent>,
    pub animated: bool,
}

#[derive(Resource, Default, Reflect)]
pub struct Structures(HashMap<String, Structure>);

impl Deref for Structures {
    type Target = HashMap<String, Structure>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Structures {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
