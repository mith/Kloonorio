use bevy::{
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
    prelude::*,
};
use bevy_rapier2d::prelude::*;
use biter::BiterPlugin;
use builder::BuilderPlugin;
use entity_tile_tracking::EntityTileTrackingPlugin;
use kloonorio_core::{types::AppState, KloonorioCorePlugins};
use kloonorio_render::KloonorioRenderPlugins;
use kloonorio_terrain::KloonorioTerrainPlugin;
use kloonorio_ui::KloonorioUiPlugin;
use loading::LoadState;
use scene_setup::SceneSetupPlugin;
use shoot::ShootPlugin;

pub mod biter;
mod builder;
mod camera;
mod craft;
mod entity_tile_tracking;
mod interact;
mod item_loader;
mod loading;
mod player;
mod player_control;
mod recipe_loader;
mod scene_setup;
mod shoot;
mod structure_loader;
mod ysort;

use crate::{
    camera::PanZoomCameraPlugin,
    craft::CraftPlugin,
    interact::{InteractPlugin, InteractionSettings},
    item_loader::ItemLoaderPlugin,
    loading::LoadingPlugin,
    player::PlayerPlugin,
    player_control::PlayerControlPlugin,
    recipe_loader::RecipeLoaderPlugin,
    structure_loader::StructureLoaderPlugin,
    ysort::YSortPlugin,
};

fn main() {
    let mut app = App::new();
    app.init_resource::<LoadState>()
        .add_state::<AppState>()
        .add_plugins((
            FrameTimeDiagnosticsPlugin,
            LogDiagnosticsPlugin::default(),
            DefaultPlugins
                .set(AssetPlugin { ..default() })
                .set(ImagePlugin::default_nearest()),
            RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(16.),
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
            PlayerControlPlugin,
            RecipeLoaderPlugin,
            StructureLoaderPlugin,
            ItemLoaderPlugin,
            LoadingPlugin,
            InteractPlugin,
            CraftPlugin,
            BuilderPlugin,
            PlayerPlugin,
            BiterPlugin,
            ShootPlugin,
        ))
        .add_plugins((
            PanZoomCameraPlugin,
            SceneSetupPlugin,
            EntityTileTrackingPlugin,
        ))
        .run();
}
