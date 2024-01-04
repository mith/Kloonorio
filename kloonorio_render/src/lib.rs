use bevy::app::{PluginGroup, PluginGroupBuilder};

mod inserter;
pub mod isometric_sprite;
pub mod item_textures;
mod transport_belt;

pub struct KloonorioRenderPlugins;

impl PluginGroup for KloonorioRenderPlugins {
    fn build(self) -> bevy::app::PluginGroupBuilder {
        PluginGroupBuilder::start::<Self>()
            .add(isometric_sprite::IsometricSpritePlugin)
            .add(transport_belt::TransportBeltRenderPlugin)
            .add(inserter::InserterRenderPlugin)
    }
}
