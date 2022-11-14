use bevy::{
    asset::{AssetLoader, LoadContext, LoadedAsset},
    prelude::*,
    reflect::TypeUuid,
    utils::BoxedFuture,
};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize, TypeUuid)]
#[uuid = "990c9ea7-3c00-4d6b-b9f0-c62b86bb9973"]
pub enum StructureComponent {
    Smelter,
    Burner,
    CraftingQueue,
    Inventory(u32),
    Source(u32),
    Output(u32),
}

#[derive(Clone, Debug, Deserialize, TypeUuid)]
#[uuid = "540f864d-3e80-4e5d-8be5-1846d7be2484"]
pub struct Structure {
    pub name: String,
    pub size: IVec2,
    pub components: Vec<StructureComponent>,
}

#[derive(Default)]
pub struct StructuresAssetLoader;

#[derive(Clone, Debug, TypeUuid)]
#[uuid = "97b2a898-da7d-4a72-a192-05e18d309950"]
pub struct StructuresAsset(pub Vec<Structure>);

impl AssetLoader for StructuresAssetLoader {
    fn load<'a>(
        &'a self,
        bytes: &'a [u8],
        load_context: &'a mut LoadContext,
    ) -> BoxedFuture<'a, Result<(), bevy::asset::Error>> {
        Box::pin(async move {
            let path = load_context.path().display().to_string();
            let _span = info_span!("Loading structures asset", path = path);
            let _enter = _span.enter();
            match ron::de::from_bytes(bytes) {
                Ok(structures) => {
                    load_context.set_default_asset(LoadedAsset::new(StructuresAsset(structures)));
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
        &["structures.ron"]
    }
}

pub struct StructureLoaderPlugin;

impl Plugin for StructureLoaderPlugin {
    fn build(&self, app: &mut App) {
        app.add_asset::<StructuresAsset>()
            .init_asset_loader::<StructuresAssetLoader>();
    }
}
