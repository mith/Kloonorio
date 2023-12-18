use std::ops::{Deref, DerefMut};

use bevy::{
    asset::{LoadedFolder, RecursiveDependencyLoadState},
    ecs::system::SystemParam,
    prelude::*,
    utils::HashMap,
};
use bevy_egui::EguiContexts;

use crate::{
    intermediate_loader::{Intermediate, IntermediateAsset},
    recipe_loader::RecipesAsset,
    structure_loader::{Structure, StructuresAsset},
    types::{AppState, GameState, Recipe},
};

#[derive(Resource, Default)]
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

#[derive(Resource, Default)]
pub struct Recipes(HashMap<String, Recipe>);

impl Deref for Recipes {
    type Target = HashMap<String, Recipe>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Recipes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource, Default)]
pub struct Icons(HashMap<String, egui::TextureId>);

impl Deref for Icons {
    type Target = HashMap<String, egui::TextureId>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Icons {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource, Default)]
pub struct Resources(HashMap<String, Intermediate>);

impl Deref for Resources {
    type Target = HashMap<String, Intermediate>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Resources {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(SystemParam)]
pub struct Definitions<'w> {
    pub structures: Res<'w, Structures>,
    pub recipes: Res<'w, Recipes>,
    pub icons: Res<'w, Icons>,
    pub resources: Res<'w, Resources>,
}

fn start_loading(asset_server: Res<AssetServer>, mut gamestate: ResMut<GameState>) {
    gamestate.recipes_handle = asset_server.load("data/base.recipes.ron");
    gamestate.structures_handle = asset_server.load("data/base.structures.ron");
    gamestate.icons_handle = asset_server.load_folder("textures/icons");
    gamestate.resources_handle = asset_server.load("data/base.resources.ron");
}

fn load_resources(
    mut gamestate: ResMut<GameState>,
    resource_assets: Res<Assets<IntermediateAsset>>,
    mut resources: ResMut<Resources>,
) {
    let resource_asset = resource_assets.get(&gamestate.resources_handle);
    if gamestate.resources_loaded || resource_asset.is_none() {
        return;
    }

    if let Some(IntermediateAsset(loaded_resource)) = resource_asset {
        resources.extend(loaded_resource.iter().map(|r| (r.name.clone(), r.clone())));
        gamestate.resources_loaded = true;
    }
}

fn load_icons(
    asset_server: Res<AssetServer>,
    mut gamestate: ResMut<GameState>,
    mut egui_context: EguiContexts,
    mut icons: ResMut<Icons>,
    loaded_folder_assets: Res<Assets<LoadedFolder>>,
) {
    if !gamestate.icons_loaded
        && asset_server.get_recursive_dependency_load_state(&gamestate.icons_handle)
            == Some(RecursiveDependencyLoadState::Loaded)
    {
        let loaded_folder = loaded_folder_assets.get(&gamestate.icons_handle).unwrap();
        for icon in &loaded_folder.handles {
            let texture_id = egui_context.add_image(icon.clone().typed::<Image>());
            if let Some(name) = asset_server
                .get_path(icon.id())
                .map(|ap| ap.path().file_stem().unwrap().to_string_lossy().to_string())
            {
                icons.insert(name, texture_id);
            }
        }
        gamestate.icons_loaded = true;
    }
}

fn load_structures(
    mut gamestate: ResMut<GameState>,
    structures_assets: Res<Assets<StructuresAsset>>,
    mut structures: ResMut<Structures>,
) {
    let structures_asset = structures_assets.get(&gamestate.structures_handle);
    if gamestate.structures_loaded || structures_asset.is_none() {
        return;
    }

    if let Some(StructuresAsset(loaded_structures)) = structures_asset {
        structures.extend(
            loaded_structures
                .iter()
                .map(|s| (s.name.clone(), s.clone())),
        );
        gamestate.structures_loaded = true;
    }
}

fn load_recipes(
    mut gamestate: ResMut<GameState>,
    recipes_assets: Res<Assets<RecipesAsset>>,
    mut recipes: ResMut<Recipes>,
) {
    let recipe_assets = recipes_assets.get(&gamestate.recipes_handle);
    if gamestate.recipes_loaded || recipe_assets.is_none() {
        return;
    }

    if let Some(RecipesAsset(loaded_recipes)) = recipe_assets {
        recipes.extend(
            loaded_recipes
                .iter()
                .map(|r| (r.name.to_string(), r.clone())),
        );
        gamestate.recipes_loaded = true;
    }
}

fn check_loading(gamestate: Res<GameState>, mut next_state: ResMut<NextState<AppState>>) {
    if gamestate.recipes_loaded
        && gamestate.structures_loaded
        && gamestate.icons_loaded
        && gamestate.resources_loaded
    {
        next_state.set(AppState::Running);
    }
}

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Structures::default())
            .insert_resource(Recipes::default())
            .insert_resource(Icons::default())
            .insert_resource(Resources::default())
            .add_systems(OnEnter(AppState::Loading), start_loading)
            .add_systems(
                Update,
                (
                    load_recipes,
                    load_structures,
                    load_icons,
                    load_resources,
                    check_loading,
                )
                    .run_if(in_state(AppState::Loading)),
            );
    }
}
