use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;

#[derive(Default)]
pub struct IntermediateAssetLoader;

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "5ccee2a9-9fcd-4a56-ba64-bb3cb24c208f"]
pub struct Intermediate {
    pub name: String,
}

#[derive(Asset, TypePath, Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "09483f6e-220b-486c-aaf2-857b4c9cab23"]
pub struct IntermediateAsset(pub Vec<Intermediate>);

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

impl AssetLoader for IntermediateAssetLoader {
    type Asset = IntermediateAsset;
    type Settings = ();
    type Error = IntermediateAssetLoaderError;
    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        _settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading resource asset", path = path);
            let _enter = _span.enter();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let intermediate_asset = ron::de::from_bytes(&buf)?;
            debug!("Finished loading");
            Ok(IntermediateAsset(intermediate_asset))
        })
    }

    fn extensions(&self) -> &[&str] {
        &["resources.ron"]
    }
}

pub struct IntermediateLoaderPlugin;

impl Plugin for IntermediateLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<IntermediateAsset>()
            .init_asset_loader::<IntermediateAssetLoader>();
    }
}
