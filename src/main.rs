use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_rapier2d::prelude::*;
use inventory::Stack;
use structure_components::StructureComponentsPlugin;

mod camera;
mod craft;
mod discrete_rotation;
mod interact;
mod inventory;
mod isometric_sprite;
mod item_loader;
mod loading;
mod picker;
mod placeable;
mod player;
mod player_movement;
mod recipe_loader;
mod structure_components;
mod structure_loader;
mod terrain;
mod types;
mod ui;
mod util;
mod ysort;

use crate::{
    camera::PanZoomCameraPlugin,
    craft::CraftPlugin,
    interact::{InteractPlugin, PlayerSettings},
    isometric_sprite::IsometricSpritePlugin,
    item_loader::ItemLoaderPlugin,
    loading::LoadingPlugin,
    picker::PickerPlugin,
    player::PlayerPlugin,
    player_movement::PlayerMovementPlugin,
    recipe_loader::RecipeLoaderPlugin,
    structure_loader::StructureLoaderPlugin,
    terrain::TerrainPlugin,
    types::{AppState, GameState, Item},
    ui::UiPlugin,
    ysort::YSortPlugin,
};

fn main() {
    let mut app = App::new();
    app.init_resource::<GameState>()
        .insert_resource(PlayerSettings {
            max_mining_distance: 20.,
        })
        .add_state::<AppState>()
        // .add_plugin(LogDiagnosticsPlugin::default())
        .insert_resource(Time::<Fixed>::from_hz(60.))
        .add_plugins((
            FrameTimeDiagnosticsPlugin,
            DefaultPlugins
                .set(AssetPlugin { ..default() })
                .set(ImagePlugin::default_nearest()),
            EguiPlugin,
            DefaultInspectorConfigPlugin,
        ))
        // .insert_resource(WorldInspectorParams {
        //     name_filter: Some("Interesting".into()),
        //     ..default()
        // })
        .register_type::<Item>()
        .insert_resource(RapierConfiguration {
            gravity: Vec2::new(0.0, 8.0),
            ..default()
        })
        .add_plugins((
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0),
            YSortPlugin,
            TerrainPlugin,
            IsometricSpritePlugin,
            PlayerMovementPlugin,
            RecipeLoaderPlugin,
            StructureLoaderPlugin,
            StructureComponentsPlugin,
            ItemLoaderPlugin,
            LoadingPlugin,
            PickerPlugin,
            UiPlugin,
            InteractPlugin,
            CraftPlugin,
            PlayerPlugin,
        ))
        .add_plugins(PanZoomCameraPlugin)
        .register_type::<Stack>()
        .add_systems(
            Update,
            (placeable::placeable, placeable::placeable_rotation)
                .run_if(in_state(AppState::Running)),
        )
        .run();
}
