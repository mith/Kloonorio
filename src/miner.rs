use bevy::{math::Vec3Swizzles, prelude::*};

use bevy_rapier2d::prelude::RapierContext;

use crate::{
    inventory::{Inventory, Output, Stack},
    is_minable,
    terrain::Terrain,
    types::{Powered, Working},
    util::{drop_stack_at_point, texture_id_to_product},
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

pub fn miner_tick(
    mut commands: Commands,
    mut miner_query: Query<(Entity, &Transform, &mut Miner), With<Powered>>,
    terrain: Terrain,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    rapier_context: Res<RapierContext>,
    mut inventories_query: Query<&mut Inventory, Without<Output>>,
    children: Query<&Children>,
) {
    for (miner_entity, transform, mut miner) in miner_query.iter_mut() {
        let span = info_span!("Miner tick", miner = ?miner_entity);
        let _enter = span.enter();
        if let Some(tile_entity) = terrain.tile_entity_at_point(transform.translation.xy()) {
            if let Some(tile_texture) = terrain.tile_texture_index(tile_entity) {
                if is_minable(tile_texture.0) {
                    if miner.timer.tick(time.delta()).just_finished() {
                        let stack = Stack::new(texture_id_to_product(tile_texture.clone()), 1);
                        info!("Produced {:?}", stack);
                        let drop_point = transform.translation - Vec3::new(0.5, 1.5, 0.);
                        info!("Dropping at {:?}", drop_point);

                        drop_stack_at_point(
                            &mut commands,
                            &rapier_context,
                            &asset_server,
                            &mut inventories_query,
                            &children,
                            stack,
                            drop_point,
                        );
                        commands.entity(tile_entity).remove::<Working>();
                    } else {
                        commands.entity(miner_entity).insert(Working);
                    }
                }
            }
        }
    }
}
