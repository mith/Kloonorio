use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_ecs_tilemap::prelude::*;
use bevy_rapier2d::prelude::Collider;

use crate::{
    inventory::Stack,
    is_minable,
    terrain::{tile_at_point, SpawnedChunk, COAL, IRON, STONE, TILE_SIZE, TREE},
    types::{Powered, Product, Working},
};

#[derive(Component)]
pub struct Miner {
    timer: Timer,
}

impl Miner {
    pub fn new(speed: f32) -> Self {
        Miner {
            timer: Timer::from_seconds(speed, TimerMode::Repeating),
        }
    }
}

fn texture_id_to_product(index: TileTextureIndex) -> Product {
    match index.0 {
        COAL => Product::Coal,
        IRON => Product::IronOre,
        STONE => Product::Stone,
        TREE => Product::Wood,
        _ => panic!("Unknown product"),
    }
}

fn product_to_texture(product: Product) -> String {
    match product {
        Product::Wood => "tree".to_string(),
        _ => "no_icon".to_string(),
    }
}

pub fn miner_tick(
    mut commands: Commands,
    mut miner_query: Query<(Entity, &Transform, &mut Miner), With<Powered>>,
    chunks_query: Query<
        (
            &Transform,
            &TileStorage,
            &TilemapSize,
            &TilemapGridSize,
            &TilemapType,
        ),
        With<SpawnedChunk>,
    >,
    tile_query: Query<&TileTextureIndex>,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
) {
    for (miner_entity, transform, mut miner) in miner_query.iter_mut() {
        let span = info_span!("Miner tick", miner = ?miner_entity);
        let _enter = span.enter();
        if let Some(tile_entity) = tile_at_point(transform.translation.xy(), &chunks_query) {
            if let Ok(tile_texture) = tile_query.get(tile_entity) {
                if is_minable(tile_texture.0) {
                    if miner.timer.tick(time.delta()).just_finished() {
                        let product = texture_id_to_product(tile_texture.clone());
                        info!("Produced {:?}", product);
                        let dump_point =
                            transform.translation + Vec3::new(TILE_SIZE.x, TILE_SIZE.y, 0.) * 2.;
                        info!(
                            "Dumping at {:?} (miner at {:?})",
                            dump_point, transform.translation
                        );

                        let path =
                            format!("textures/icons/{}.png", product_to_texture(product.clone()));
                        info!("Loading texture at {:?}", path);
                        commands.spawn((
                            Stack::new(product.clone(), 1),
                            Collider::cuboid(3., 3.),
                            SpriteBundle {
                                texture: asset_server.load(path),
                                transform: Transform::from_translation(dump_point),
                                sprite: Sprite {
                                    custom_size: Some(Vec2::new(6., 6.)),
                                    ..default()
                                },
                                ..default()
                            },
                        ));

                        commands.entity(miner_entity).insert(Working);
                    }
                }
            }
        }
    }
}
