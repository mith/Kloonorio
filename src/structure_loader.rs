use bevy::{
    asset::{io::Reader, AssetLoader, AsyncReadExt, LoadContext},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;

use crate::structure_components::StructureComponent;

#[derive(Default)]
pub struct StructuresAssetLoader;

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "540f864d-3e80-4e5d-8be5-1846d7be2484"]
pub struct Structure {
    pub name: String,
    pub size: IVec2,
    pub collider: Vec2,
    pub sides: u32,
    pub components: Vec<StructureComponent>,
    pub animated: bool,
}

#[derive(Asset, TypePath, Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "97b2a898-da7d-4a72-a192-05e18d309950"]
pub struct StructuresAsset(pub Vec<Structure>);

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum StructuresAssetLoaderError {
    /// An [IO](std::io) Error.
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    /// A [Ron](ron) Error.
    #[error("Could not parse RON: {0}")]
    RonSpannedError(#[from] ron::error::SpannedError),
}

impl AssetLoader for StructuresAssetLoader {
    type Asset = StructuresAsset;
    type Settings = ();
    type Error = StructuresAssetLoaderError;

    fn load<'a>(
        &'a self,
        reader: &'a mut Reader,
        settings: &'a Self::Settings,
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<Self::Asset, Self::Error>> {
        let _ = settings;
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading structures asset", path = path);
            let _enter = _span.enter();
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf).await?;
            let intermediate_asset = ron::de::from_bytes(&buf)?;
            debug!("Finished loading");
            Ok(StructuresAsset(intermediate_asset))
        })
    }

    fn extensions(&self) -> &[&str] {
        &["structures.ron"]
    }
}

pub struct StructureLoaderPlugin;

impl Plugin for StructureLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<StructuresAsset>()
            .init_asset_loader::<StructuresAssetLoader>();
    }
}
