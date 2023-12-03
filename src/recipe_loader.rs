use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;

use crate::types::Recipe;

#[derive(Default)]
pub struct RecipesAssetLoader;

#[derive(Asset, TypePath, Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "6b92cebe-2ec6-4e22-b85d-499873f9c22c"]
pub struct RecipesAsset(pub Vec<Recipe>);

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum RecipeAssetLoaderError {
    /// An [IO](std::io) Error.
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    /// A [Ron](ron) Error.
    #[error("Could not parse RON: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),
}

impl AssetLoader for RecipesAssetLoader {
    type Asset = RecipesAsset;
    type Error = RecipeAssetLoaderError;
    type Settings = ();
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<RecipesAsset, Self::Error>> {
        let _ = settings;
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading recipes asset", path = path);
            let _enter = _span.enter();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let recipes_asset = ron::de::from_bytes(&buf)?;
            debug!("Finished loading");
            Ok(RecipesAsset(recipes_asset))
        })
    }

    fn extensions(&self) -> &[&str] {
        &["recipes.ron"]
    }
}

pub struct RecipeLoaderPlugin;

impl Plugin for RecipeLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<RecipesAsset>()
            .init_asset_loader::<RecipesAssetLoader>();
    }
}
