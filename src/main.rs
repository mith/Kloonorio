use bevy::{diagnostic::FrameTimeDiagnosticsPlugin, prelude::*, render::camera::CameraPlugin};
use bevy_egui::EguiPlugin;
use bevy_inspector_egui::DefaultInspectorConfigPlugin;
use bevy_rapier2d::prelude::*;
use craft::CraftPlugin;

use inserter::{burner_inserter_tick, inserter_tick};
use interact::{InteractPlugin, PlayerSettings};
use isometric_sprite::IsometricSpritePlugin;
use picker::PickerPlugin;
use player::PlayerPlugin;
use transport_belt::TransportBeltPlugin;
use ui::UiPlugin;

mod burner;
mod camera;
mod craft;
mod discrete_rotation;
mod inserter;
mod interact;
mod intermediate_loader;
mod inventory;
mod isometric_sprite;
mod loading;
mod miner;
mod picker;
mod placeable;
mod player;
mod player_movement;
mod recipe_loader;
mod smelter;
mod structure_loader;
mod terrain;
mod transport_belt;
mod types;
mod ui;
mod util;

use crate::{
    burner::{burner_load, burner_tick},
    intermediate_loader::IntermediateLoaderPlugin,
    loading::LoadingPlugin,
    miner::miner_tick,
    player_movement::PlayerMovementPlugin,
    recipe_loader::RecipeLoaderPlugin,
    smelter::smelter_tick,
    structure_loader::StructureLoaderPlugin,
    terrain::TerrainPlugin,
    types::{AppState, GameState, Product},
};

fn main() {
    let mut app = App::new();
    app.init_resource::<GameState>()
        .insert_resource(PlayerSettings {
            max_mining_distance: 20.,
        })
        .add_state::<AppState>()
        // .add_plugin(LogDiagnosticsPlugin::default())
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin { ..default() })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins(EguiPlugin)
        // .insert_resource(WorldInspectorParams {
        //     name_filter: Some("Interesting".into()),
        //     ..default()
        // })
        .add_plugins(DefaultInspectorConfigPlugin)
        .register_type::<Product>()
        .insert_resource(RapierConfiguration {
            gravity: Vec2::new(0.0, 8.0),
            ..default()
        })
        .add_plugins((
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0),
            RapierDebugRenderPlugin::default(),
            TerrainPlugin,
            IsometricSpritePlugin,
            PlayerMovementPlugin,
            RecipeLoaderPlugin,
            StructureLoaderPlugin,
            IntermediateLoaderPlugin,
            TransportBeltPlugin,
            LoadingPlugin,
            PickerPlugin,
            UiPlugin,
            InteractPlugin,
            CraftPlugin,
            PlayerPlugin,
        ))
        .add_plugins(CameraPlugin)
        .add_systems(
            Update,
            (
                smelter_tick,
                burner_tick,
                burner_load,
                miner_tick,
                inserter_tick,
                burner_inserter_tick,
                placeable::placeable,
                placeable::placeable_rotation,
            )
                .run_if(in_state(AppState::Running)),
        )
        .run();
}
