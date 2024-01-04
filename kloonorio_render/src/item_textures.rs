use bevy::{
    asset::Handle, ecs::system::Resource, render::texture::Image, sprite::TextureAtlas,
    utils::HashMap,
};

#[derive(Resource)]
pub struct ItemTextures {
    pub images: HashMap<String, Handle<Image>>,
    pub item_texture_index: HashMap<String, usize>,
    pub texture_atlas_handle: Handle<TextureAtlas>,
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
