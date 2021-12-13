use bevy::{input::mouse::MouseMotion, prelude::*};

#[derive(Default, Clone)]
pub struct SpriteHandles {
    pub handles: Vec<HandleUntyped>,
    pub atlas_loaded: bool,
}

#[derive(Default)]
pub struct CursorState {
    pub under_cursor: Option<usize>,
}

#[derive(Default)]
pub struct GameState {
    pub map_loaded: bool,
    pub spawned: bool,
}

pub struct Player;

#[derive(Hash, Eq, PartialEq, Debug)]
pub enum Resource {
    Coal,
}
