use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;

use crate::types::Item;

#[derive(Default)]
pub struct ItemAssetLoader;
#[derive(Asset, TypePath, Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "09483f6e-220b-486c-aaf2-857b4c9cab23"]
pub struct ItemAsset(pub Vec<Item>);

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum IntermediateAssetLoaderError {
    /// An [IO](std::io) Error.
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    /// A [Ron](ron) Error.
    #[error("Could not parse RON: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),
}

impl AssetLoader for ItemAssetLoader {
    type Asset = ItemAsset;
    type Settings = ();
    type Error = IntermediateAssetLoaderError;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        let _ = settings;
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading resource asset", path = path);
            let _enter = _span.enter();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let intermediate_asset = ron::de::from_bytes(&buf)?;
            debug!("Finished loading");
            Ok(ItemAsset(intermediate_asset))
        })
    }

    fn extensions(&self) -> &[&str] {
        &["resources.ron"]
    }
}

pub struct ItemLoaderPlugin;

impl Plugin for ItemLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<ItemAsset>()
            .init_asset_loader::<ItemAssetLoader>();
    }
}
