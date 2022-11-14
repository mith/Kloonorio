use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;

use crate::types::Resource;

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "1ca725c1-5a0d-484f-8d04-a5a42960e208"]
pub struct Recipe {
    pub materials: Vec<(Resource, u32)>,
    pub products: Vec<(Resource, u32)>,
    pub crafting_time: f32,
    pub name: String,
}

#[derive(Default)]
pub struct RecipeAssetLoader;

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "6b92cebe-2ec6-4e22-b85d-499873f9c22c"]
pub struct RecipeAsset(pub Vec<Recipe>);

impl AssetLoader for RecipeAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading recipes asset", path = path);
            let _enter = _span.enter();
            match ron::de::from_bytes(bytes) {
                Ok(recipes) => {
                    load_context.set_default_asset(LoadedAsset::new(RecipeAsset(recipes)));
                    debug!("Finished loading");
                    Ok(())
                }
                Err(err) => {
                    error!(error = err.to_string());
                    Err(bevy::asset::Error::new(err))
                }
            }
        })
    }

    fn extensions(&self) -> &[&str] {
        &["recipes.ron"]
    }
}

pub struct RecipeLoaderPlugin;

impl Plugin for RecipeLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<RecipeAsset>()
            .init_asset_loader::<RecipeAssetLoader>();
    }
}
