use bevy::{
    app::{App, Plugin, Startup},
    asset::AssetServer,
    ecs::system::{Commands, Res},
    prelude::default,
};

use crate::terrain::{
    terrain_generator::{NoiseChunkGenerator, TerrainGenerator},
    Terrain, TerrainBundle,
};

pub struct SceneSetupPlugin;

impl Plugin for SceneSetupPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_terrain);
    }
}

fn setup_terrain(mut commands: Commands, asset_server: Res<AssetServer>) {
    let chunk_generator = NoiseChunkGenerator::new(1234567);
    let terrain_generator = TerrainGenerator::new(Box::new(chunk_generator));
    let terrain_texture = asset_server.load("textures/terrain.png");

    commands.spawn(TerrainBundle {
        terrain: Terrain::new(terrain_texture),
        generator: terrain_generator,
        ..default()
    });
}
