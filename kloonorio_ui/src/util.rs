use bevy::ecs::system::{Res, SystemParam};
use kloonorio_core::{item::Items, recipe::Recipes, structure::Structures};

use crate::icon::Icons;

#[derive(SystemParam)]
pub struct Definitions<'w> {
    pub structures: Res<'w, Structures>,
    pub recipes: Res<'w, Recipes>,
    pub icons: Res<'w, Icons>,
    pub items: Res<'w, Items>,
}
