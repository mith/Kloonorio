use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};

use crate::{
    inventory::{Inventory, Stack},
    terrain::TILE_SIZE,
    types::Powered,
    util::{drop_stack_at_point, take_stack_from_entity_inventory},
};

#[derive(Component)]
pub struct Inserter {
    holding: Option<Stack>,
    speed: f32,
    capacity: u32,
    timer: Timer,
}

impl Inserter {
    pub fn new(speed: f32, capacity: u32) -> Self {
        Inserter {
            holding: None,
            speed,
            capacity,
            timer: Timer::from_seconds(speed, TimerMode::Repeating),
        }
    }
}

pub fn inserter_tick(
    mut commands: Commands,
    mut inserter_query: Query<(Entity, &Transform, &mut Inserter), With<Powered>>,
    children: Query<&Children>,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    mut inventories_query: Query<&mut Inventory>,
    asset_server: Res<AssetServer>,
) {
    for (inserter_entity, inserter_transform, mut inserter) in &mut inserter_query {
        let span = info_span!("Inserter tick", inserter = ?inserter_entity);
        let _enter = span.enter();

        if let Some(holding) = inserter.holding.clone() {
            if inserter.timer.tick(time.delta()).just_finished() {
                let drop_point = inserter_transform.translation
                    + inserter_transform.rotation * Vec3::new(TILE_SIZE.x, 0., 0.);

                info!("Dropping {:?}", holding);

                drop_stack_at_point(
                    &mut commands,
                    &rapier_context,
                    &asset_server,
                    &mut inventories_query,
                    &children,
                    holding,
                    drop_point,
                );

                inserter.holding = None;
            }
        } else {
            let pickup_point = inserter_transform.translation
                + inserter_transform.rotation * Vec3::new(-TILE_SIZE.x, 0., 0.);
            if let Some(collider_entity) = rapier_context.intersection_with_shape(
                pickup_point.xy(),
                0.,
                &Collider::ball(2.),
                QueryFilter::new(),
            ) {
                inserter.holding = take_stack_from_entity_inventory(
                    &mut inventories_query,
                    collider_entity,
                    &children,
                    inserter.capacity,
                );
            }
        }
    }
}
