use bevy::{input::mouse::MouseMotion, prelude::*};

#[derive(Clone, PartialEq, Eq, Component, Debug, Hash)]
pub enum AppState {
    Setup,
    Running
}

#[derive(Default)]
#[derive(Component)]
pub struct CursorState {
    pub under_cursor: Option<usize>,
}

#[derive(Default)]
#[derive(Component)]
pub struct GameState {
    pub map_loaded: bool,
    pub spawned: bool,
}

#[derive(Component)]
pub struct Player;

#[derive(Hash, Eq, PartialEq, Debug)]
pub enum Resource {
    Coal,
}
