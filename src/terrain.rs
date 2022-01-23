use crate::types::GameState;
use bevy::{
    asset::LoadState, prelude::*, render::render_resource::TextureUsages,
    sprite::TextureAtlasBuilder,
};
use bevy_ecs_tilemap::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PluginState {
    Setup,
    Finished,
}

#[derive(Default)]
struct TextureHandles {
    handles: Vec<HandleUntyped>,
}

fn load_textures(mut texture_handles: ResMut<TextureHandles>, asset_server: Res<AssetServer>) {
    texture_handles.handles = asset_server.load_folder("textures/terrain").unwrap();
}

fn check_textures(
    mut state: ResMut<State<PluginState>>,
    texture_handles: ResMut<TextureHandles>,
    asset_server: Res<AssetServer>,
) {
    if let LoadState::Loaded =
        asset_server.get_group_load_state(texture_handles.handles.iter().map(|handle| handle.id))
    {
        state.set(PluginState::Finished).unwrap();
    }
}

fn setup(
    mut commands: Commands,
    texture_handles: Res<TextureHandles>,
    mut textures: ResMut<Assets<Image>>,
    mut map_query: MapQuery,
) {
    // Lets load all our textures from our folder!
    let mut texture_atlas_builder = TextureAtlasBuilder::default();

    for handle in texture_handles.handles.iter() {
        let texture = textures.get(handle).unwrap();
        texture_atlas_builder.add_texture(handle.clone_weak().typed::<Image>(), &texture);
    }

    let texture_atlas = texture_atlas_builder.finish(&mut textures).unwrap();

    let map_entity = commands.spawn().id();
    let mut map = Map::new(0u16, map_entity);

    let layer_settings = LayerSettings::new(
        MapSize(2, 2),
        ChunkSize(8, 8),
        TileSize(16.0, 16.0),
        TextureSize(96.0, 16.0),
    );

    let center = layer_settings.get_pixel_center();

    let (mut ground_layer_builder, ground_layer) =
        LayerBuilder::new(&mut commands, layer_settings, 0u16, 0u16);
    map.add_layer(&mut commands, 0u16, ground_layer);

    ground_layer_builder.set_all(TileBundle {
        tile: Tile {
            texture_index: 1,
            ..Default::default()
        },
        ..Default::default()
    });

    map_query.build_layer(&mut commands, ground_layer_builder, texture_atlas.texture);

    commands
        .entity(map_entity)
        .insert(map)
        .insert(Transform::from_xyz(-center.x, -center.y, 0.0))
        .insert(GlobalTransform::default());
}

pub fn set_texture_filters_to_nearest(
    mut texture_events: EventReader<AssetEvent<Image>>,
    mut textures: ResMut<Assets<Image>>,
) {
    // quick and dirty, run this for all textures anytime a texture is created.
    for event in texture_events.iter() {
        match event {
            AssetEvent::Created { handle } => {
                if let Some(mut texture) = textures.get_mut(handle) {
                    texture.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
                        | TextureUsages::COPY_SRC
                        | TextureUsages::COPY_DST;
                }
            }
            _ => (),
        }
    }
}

pub struct TerrainPlugin;
impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TextureHandles>()
            .add_state(PluginState::Setup)
            .add_system_set(SystemSet::on_enter(PluginState::Setup).with_system(load_textures))
            .add_system_set(SystemSet::on_update(PluginState::Setup).with_system(check_textures))
            .add_system_set(SystemSet::on_enter(PluginState::Finished).with_system(setup))
            .add_system(set_texture_filters_to_nearest);
    }
}
