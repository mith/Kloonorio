use std::ops::{Deref, DerefMut};

use bevy::{
    asset::{LoadedFolder, RecursiveDependencyLoadState},
    ecs::system::SystemParam,
    prelude::*,
    transform::commands,
    utils::HashMap,
};
use bevy_egui::EguiContexts;

use crate::{
    item_loader::ItemAsset,
    recipe_loader::RecipesAsset,
    structure_loader::{Structure, StructuresAsset},
    types::{AppState, GameState, Item, Recipe},
};

#[derive(Resource, Default, Reflect)]
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

#[derive(Resource, Default, Reflect)]
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

#[derive(Resource, Default, Reflect)]
pub struct Items(HashMap<String, Item>);

impl Deref for Items {
    type Target = HashMap<String, Item>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Items {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource)]
pub struct ItemTextures {
    images: HashMap<String, Handle<Image>>,
    item_texture_index: HashMap<String, usize>,
    texture_atlas_handle: Handle<TextureAtlas>,
}

impl ItemTextures {
    pub fn get_texture_index(&self, item_name: &str) -> Option<usize> {
        let item_image_name = &item_name.to_lowercase().replace(' ', "_");
        self.item_texture_index.get(item_image_name).copied()
    }

    pub fn get_texture_atlas_handle(&self) -> Handle<TextureAtlas> {
        self.texture_atlas_handle.clone()
    }
}

#[derive(SystemParam)]
pub struct Definitions<'w> {
    pub structures: Res<'w, Structures>,
    pub recipes: Res<'w, Recipes>,
    pub icons: Res<'w, Icons>,
    pub items: Res<'w, Items>,
    pub item_textures: Res<'w, ItemTextures>,
}

fn start_loading(asset_server: Res<AssetServer>, mut gamestate: ResMut<GameState>) {
    gamestate.recipes_handle = asset_server.load("data/base.recipes.ron");
    gamestate.structures_handle = asset_server.load("data/base.structures.ron");
    gamestate.icons_handle = asset_server.load_folder("textures/icons");
    gamestate.resources_handle = asset_server.load("data/base.resources.ron");
}

fn load_resources(
    mut gamestate: ResMut<GameState>,
    item_assets: Res<Assets<ItemAsset>>,
    mut resources: ResMut<Items>,
) {
    let item_asset = item_assets.get(&gamestate.resources_handle);
    if gamestate.items_loaded || item_asset.is_none() {
        return;
    }

    if let Some(ItemAsset(loaded_resource)) = item_asset {
        resources.extend(loaded_resource.iter().map(|r| (r.to_string(), r.clone())));
        gamestate.items_loaded = true;
    }
}

fn load_item_textures_and_icons(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut gamestate: ResMut<GameState>,
    mut egui_context: EguiContexts,
    mut icons: ResMut<Icons>,
    loaded_folder_assets: Res<Assets<LoadedFolder>>,
    mut textures: ResMut<Assets<Image>>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    if !gamestate.icons_loaded
        && asset_server.get_recursive_dependency_load_state(&gamestate.icons_handle)
            == Some(RecursiveDependencyLoadState::Loaded)
    {
        let loaded_folder = loaded_folder_assets.get(&gamestate.icons_handle).unwrap();
        let mut texture_atlas_builder = TextureAtlasBuilder::default();
        let mut item_images = HashMap::new();

        for icon in &loaded_folder.handles {
            let item_texture = icon.clone().typed::<Image>();
            let texture_id = egui_context.add_image(item_texture.clone());
            if let Some(name) = asset_server
                .get_path(icon.id())
                .map(|ap| ap.path().file_stem().unwrap().to_string_lossy().to_string())
            {
                icons.insert(name.clone(), texture_id);
                item_images.insert(name, item_texture.clone());
                let Some(texture) = textures.get(item_texture.id()) else {
                    warn!(
                        "{:?} did not resolve to an `Image` asset",
                        item_texture.path().unwrap()
                    );
                    continue;
                };
                texture_atlas_builder.add_texture(item_texture.id(), texture)
            }
        }
        let texture_atlas = texture_atlas_builder.finish(&mut textures).unwrap();
        let texture_atlas_handle = texture_atlases.add(texture_atlas.clone());
        let item_texture_index: HashMap<String, usize> = item_images
            .iter()
            .map(|(item_name, item_image_handle)| {
                (
                    item_name.clone(),
                    texture_atlas
                        .get_texture_index(item_image_handle.id())
                        .unwrap(),
                )
            })
            .collect();
        commands.insert_resource(ItemTextures {
            images: item_images,
            texture_atlas_handle,
            item_texture_index,
        });
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
        && gamestate.items_loaded
    {
        next_state.set(AppState::Running);
    }
}

pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Structures>()
            .register_type::<Recipes>()
            .register_type::<Items>()
            .init_resource::<Structures>()
            .init_resource::<Recipes>()
            .init_resource::<Icons>()
            .init_resource::<Items>()
            .add_systems(OnEnter(AppState::Loading), start_loading)
            .add_systems(
                Update,
                (
                    load_recipes,
                    load_structures,
                    load_item_textures_and_icons,
                    load_resources,
                    check_loading,
                )
                    .run_if(in_state(AppState::Loading)),
            );
    }
}
