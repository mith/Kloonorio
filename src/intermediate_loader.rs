use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
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

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "09483f6e-220b-486c-aaf2-857b4c9cab23"]
pub struct IntermediateAsset(pub Vec<Intermediate>);

impl AssetLoader for IntermediateAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading resource asset", path = path);
            let _enter = _span.enter();
            match ron::de::from_bytes(bytes) {
                Ok(resources) => {
                    load_context.set_default_asset(LoadedAsset::new(IntermediateAsset(resources)));
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
        &["resources.ron"]
    }
}

pub struct IntermediateLoaderPlugin;

impl Plugin for IntermediateLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<IntermediateAsset>()
            .init_asset_loader::<IntermediateAssetLoader>();
    }
}
