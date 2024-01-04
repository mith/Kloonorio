use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};
use bevy_rapier2d::prelude::*;
use builder::BuilderPlugin;
use entity_tile_tracking::EntityTileTrackingPlugin;
use kloonorio_core::{types::AppState, KloonorioCorePlugins};
use kloonorio_render::KloonorioRenderPlugins;
use kloonorio_terrain::KloonorioTerrainPlugin;
use kloonorio_ui::KloonorioUiPlugin;
use loading::LoadState;
use scene_setup::SceneSetupPlugin;

mod builder;
mod camera;
mod craft;
mod entity_tile_tracking;
mod interact;
mod item_loader;
mod loading;
mod placeable;
mod player;
mod player_movement;
mod recipe_loader;
mod scene_setup;
mod structure_loader;
mod ysort;

use crate::{
    camera::PanZoomCameraPlugin,
    craft::CraftPlugin,
    interact::{InteractPlugin, PlayerSettings},
    item_loader::ItemLoaderPlugin,
    loading::LoadingPlugin,
    player::PlayerPlugin,
    player_movement::PlayerMovementPlugin,
    recipe_loader::RecipeLoaderPlugin,
    structure_loader::StructureLoaderPlugin,
    ysort::YSortPlugin,
};

fn main() {
    let mut app = App::new();
    app.init_resource::<LoadState>()
        .insert_resource(PlayerSettings {
            max_mining_distance: 20.,
        })
        .add_state::<AppState>()
        .add_plugins((
            FrameTimeDiagnosticsPlugin,
            LogDiagnosticsPlugin::default(),
            DefaultPlugins
                .set(AssetPlugin { ..default() })
                .set(ImagePlugin::default_nearest()),
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(1.0),
        ))
        .insert_resource(RapierConfiguration {
            gravity: Vec2::new(0.0, 8.0),
            ..default()
        })
        .add_plugins((
            KloonorioCorePlugins,
            KloonorioTerrainPlugin,
            KloonorioUiPlugin,
            KloonorioRenderPlugins,
        ))
        .add_plugins((
            YSortPlugin,
            PlayerMovementPlugin,
            RecipeLoaderPlugin,
            StructureLoaderPlugin,
            ItemLoaderPlugin,
            LoadingPlugin,
            InteractPlugin,
            CraftPlugin,
            BuilderPlugin,
            PlayerPlugin,
        ))
        .add_plugins((
            PanZoomCameraPlugin,
            SceneSetupPlugin,
            EntityTileTrackingPlugin,
        ))
        .add_systems(
            Update,
            (placeable::placeable, placeable::placeable_rotation)
                .run_if(in_state(AppState::Running)),
        )
        .run();
}
