use bevy::{input::mouse::MouseMotion, prelude::*};

#[derive(Clone, PartialEq, Eq, Component, Debug, Hash)]
pub enum AppState {
    Setup,
    Running,
}

#[derive(Default, Component)]
pub struct CursorState {
    pub under_cursor: Option<usize>,
}

#[derive(Default, Component)]
pub struct GameState {
    pub map_loaded: bool,
    pub spawned: bool,
}

#[derive(Component)]
pub struct Player;

#[derive(Hash, Eq, PartialEq, Debug, Clone, Copy)]
pub enum Resource {
    Coal,
    Iron,
    Wood,
    Stone,
    StoneFurnace,
    IronPlate,
}
