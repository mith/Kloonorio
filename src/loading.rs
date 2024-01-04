use bevy::{
    asset::{LoadedFolder, RecursiveDependencyLoadState},
    prelude::*,
    utils::HashMap,
};
use bevy_egui::EguiContexts;
use kloonorio_core::{item::Items, recipe::Recipes, structure::Structures, types::AppState};
use kloonorio_render::item_textures::ItemTextures;
use kloonorio_ui::icon::Icons;

use crate::{
    item_loader::ItemAsset, recipe_loader::RecipesAsset, structure_loader::StructuresAsset,
};

#[derive(Default, Resource, Reflect)]
pub struct LoadState {
    pub map_loaded: bool,
    pub spawned: bool,
    pub recipes_handle: Handle<RecipesAsset>,
    pub recipes_loaded: bool,
    pub structures_handle: Handle<StructuresAsset>,
    pub structures_loaded: bool,
    pub icons_loaded: bool,
    pub icons_handle: Handle<LoadedFolder>,
    pub items_loaded: bool,
    pub resources_handle: Handle<ItemAsset>,
    pub item_textures_loaded: bool,
}

fn start_loading(asset_server: Res<AssetServer>, mut loadstate: ResMut<LoadState>) {
    loadstate.recipes_handle = asset_server.load("data/base.recipes.ron");
    loadstate.structures_handle = asset_server.load("data/base.structures.ron");
    loadstate.icons_handle = asset_server.load_folder("textures/icons");
    loadstate.resources_handle = asset_server.load("data/base.resources.ron");
}

fn load_resources(
    mut loadstate: ResMut<LoadState>,
    item_assets: Res<Assets<ItemAsset>>,
    mut resources: ResMut<Items>,
) {
    let item_asset = item_assets.get(&loadstate.resources_handle);
    if loadstate.items_loaded || item_asset.is_none() {
        return;
    }

    if let Some(ItemAsset(loaded_resource)) = item_asset {
        resources.extend(loaded_resource.iter().map(|r| (r.to_string(), r.clone())));
        loadstate.items_loaded = true;
    }
}

fn load_item_icons(
    asset_server: Res<AssetServer>,
    mut loadstate: ResMut<LoadState>,
    mut egui_context: EguiContexts,
    mut icons: ResMut<Icons>,
    loaded_folder_assets: Res<Assets<LoadedFolder>>,
) {
    if !loadstate.icons_loaded
        && asset_server.get_recursive_dependency_load_state(&loadstate.icons_handle)
            == Some(RecursiveDependencyLoadState::Loaded)
    {
        let loaded_folder = loaded_folder_assets.get(&loadstate.icons_handle).unwrap();

        for icon in &loaded_folder.handles {
            let item_texture = icon.clone().typed::<Image>();
            let texture_id = egui_context.add_image(item_texture.clone());
            if let Some(name) = asset_server
                .get_path(icon.id())
                .map(|ap| ap.path().file_stem().unwrap().to_string_lossy().to_string())
            {
                icons.insert(name.clone(), texture_id);
            }
        }

        loadstate.icons_loaded = true;
    }
}

fn load_item_textures(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut loadstate: ResMut<LoadState>,
    loaded_folder_assets: Res<Assets<LoadedFolder>>,
    mut textures: ResMut<Assets<Image>>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    if !loadstate.item_textures_loaded
        && asset_server.get_recursive_dependency_load_state(&loadstate.icons_handle)
            == Some(RecursiveDependencyLoadState::Loaded)
    {
        let loaded_folder = loaded_folder_assets.get(&loadstate.icons_handle).unwrap();
        let mut texture_atlas_builder = TextureAtlasBuilder::default();
        let mut item_images = HashMap::new();

        for icon in &loaded_folder.handles {
            let item_texture = icon.clone().typed::<Image>();
            if let Some(name) = asset_server
                .get_path(icon.id())
                .map(|ap| ap.path().file_stem().unwrap().to_string_lossy().to_string())
            {
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
        loadstate.item_textures_loaded = true;
    }
}

fn load_structures(
    mut loadstate: ResMut<LoadState>,
    structures_assets: Res<Assets<StructuresAsset>>,
    mut structures: ResMut<Structures>,
) {
    let structures_asset = structures_assets.get(&loadstate.structures_handle);
    if loadstate.structures_loaded || structures_asset.is_none() {
        return;
    }

    if let Some(StructuresAsset(loaded_structures)) = structures_asset {
        structures.extend(
            loaded_structures
                .iter()
                .map(|s| (s.name.clone(), s.clone())),
        );
        loadstate.structures_loaded = true;
    }
}

fn load_recipes(
    mut loadstate: ResMut<LoadState>,
    recipes_assets: Res<Assets<RecipesAsset>>,
    mut recipes: ResMut<Recipes>,
) {
    let recipe_assets = recipes_assets.get(&loadstate.recipes_handle);
    if loadstate.recipes_loaded || recipe_assets.is_none() {
        return;
    }

    if let Some(RecipesAsset(loaded_recipes)) = recipe_assets {
        recipes.extend(
            loaded_recipes
                .iter()
                .map(|r| (r.name.to_string(), r.clone())),
        );
        loadstate.recipes_loaded = true;
    }
}

fn check_loading(loadstate: Res<LoadState>, mut next_state: ResMut<NextState<AppState>>) {
    if loadstate.recipes_loaded
        && loadstate.structures_loaded
        && loadstate.icons_loaded
        && loadstate.items_loaded
        && loadstate.item_textures_loaded
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
                    load_item_icons,
                    load_resources,
                    load_item_textures,
                    check_loading,
                )
                    .run_if(in_state(AppState::Loading)),
            );
    }
}
