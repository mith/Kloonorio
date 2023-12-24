use std::collections::VecDeque;

use bevy::{math::Vec3Swizzles, prelude::*};
use bevy_rapier2d::prelude::{Collider, QueryFilter, RapierContext};

use crate::{
    discrete_rotation::DiscreteRotation,
    inventory::Stack,
    types::{AppState, Item},
    util::product_to_texture,
};

use super::inserter::Dropoff;

// TODO: Right now there's a fixed 3 slots for simplicity, but it might be interesting to merge
// adjacent belts by pre- or appending the slots from newly constructed belts instead of
// creating separate entities

#[derive(Component, Reflect)]
pub struct TransportBelt {
    slots: VecDeque<Option<Item>>,
    dropoff: Entity,
}

impl TransportBelt {
    pub fn new(dropoff: Entity) -> Self {
        TransportBelt {
            slots: VecDeque::from(vec![None, None, None]),
            dropoff,
        }
    }

    pub fn can_add(&self, slot: usize) -> bool {
        self.slots[slot].is_none()
    }

    /// Add a stack to the belt at the given slot. Returns true if the stack was added
    pub fn add(&mut self, slot: usize, stack: Item) -> bool {
        if self.can_add(slot) {
            self.slots[slot] = Some(stack);
            true
        } else {
            false
        }
    }

    pub fn slot(&self, slot: usize) -> Option<&Option<Item>> {
        self.slots.get(slot)
    }

    pub fn slot_mut(&mut self, slot: usize) -> Option<&mut Option<Item>> {
        self.slots.get_mut(slot)
    }

    pub fn slots(&self) -> impl Iterator<Item = &Option<Item>> {
        self.slots.iter()
    }
}

#[derive(Resource)]
pub struct TransportBeltTimer(Timer);

pub fn transport_belt_tick(
    rapier_context: Res<RapierContext>,
    transport_belt_query: Query<Entity, With<TransportBelt>>,
    mut belts_query: Query<(&mut TransportBelt, &DiscreteRotation)>,
    mut belt_timer: ResMut<TransportBeltTimer>,
    dropoff_query: Query<&GlobalTransform, With<Dropoff>>,
    time: Res<Time>,
) {
    if !belt_timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for entity in &transport_belt_query {
        let span = info_span!("Transport belt tick", entity = ?entity);
        let _enter = span.enter();

        let (last_slot, dropoff_entity, compass_direction) = belts_query
            .get(entity)
            .map(|b| {
                (
                    b.0.slots.back().expect("Belt should have slots"),
                    b.0.dropoff,
                    b.1.compass_direction(),
                )
            })
            .unwrap();

        // First handle the last slot
        if let Some(product) = last_slot {
            // Transfer to other belt if possible
            let drop_point = dropoff_query.get(dropoff_entity).unwrap().translation();

            let stack = Stack::new(product.clone(), 1);
            let transfered: bool = rapier_context
                .intersection_with_shape(
                    drop_point.xy(),
                    0.,
                    &Collider::ball(0.2),
                    QueryFilter::new().exclude_sensors(),
                )
                .map(|e| {
                    belts_query
                        .get_mut(e)
                        .as_mut()
                        .map(|(b, r)| {
                            let slot = {
                                if r.compass_direction() == compass_direction {
                                    0
                                } else {
                                    1
                                }
                            };
                            b.add(slot, stack.item.clone())
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            if transfered {
                debug!("Transfered item to other belt");
                // Product was transfered to other belt, remove from current belt
                *belts_query
                    .get_mut(entity)
                    .as_mut()
                    .unwrap()
                    .0
                    .slots
                    .back_mut()
                    .expect("Belt should have slots") = None;
                // Rotate the belt 1 slot to the right
                belts_query
                    .get_mut(entity)
                    .as_mut()
                    .unwrap()
                    .0
                    .slots
                    .rotate_right(1);
            } else {
                debug!("Belt full");
                // Could not transfer to other belt, shift items if there is space
                // instead of rotating the belt
                if let Ok((belt, _)) = belts_query.get_mut(entity).as_mut() {
                    for i in (0..belt.slots.len()).rev().skip(1) {
                        if belt.slots[i + 1].is_none() && belt.slots[i].is_some() {
                            belt.slots[i + 1] = belt.slots[i].clone();
                            belt.slots[i] = None;
                        }
                    }
                }
            }
        } else {
            belts_query
                .get_mut(entity)
                .as_mut()
                .unwrap()
                .0
                .slots
                .rotate_right(1);
        }
    }
}

#[derive(Component, Reflect)]
pub struct BeltItem;

pub fn create_transport_belt_sprites(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    transport_belt_query: Query<(Entity, &TransportBelt)>,
    belt_item_query: Query<Entity, With<BeltItem>>,
) {
    for belt_item in belt_item_query.iter() {
        commands.entity(belt_item).despawn_recursive();
    }

    for (transport_belt_entity, transport_belt) in transport_belt_query.iter() {
        for (i, slot) in transport_belt.slots.iter().enumerate() {
            if let Some(product) = slot {
                let image: Handle<Image> = asset_server.load(format!(
                    "textures/icons/{}.png",
                    product_to_texture(product)
                ));
                let sprite_transform = Transform::from_xyz(0., (i as i32 - 1) as f32 * 0.3, 1.);
                let slot_sprite = commands
                    .spawn((
                        BeltItem,
                        SpriteBundle {
                            transform: sprite_transform,
                            texture: image,
                            sprite: Sprite {
                                // Pass the custom size
                                custom_size: Some(Vec2::new(0.4, 0.4)),
                                ..default()
                            },
                            ..default()
                        },
                    ))
                    .id();
                commands
                    .entity(transport_belt_entity)
                    .add_child(slot_sprite);
            }
        }
    }
}

pub struct TransportBeltPlugin;

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
struct BeltItemRenderSet;

impl Plugin for TransportBeltPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(TransportBeltTimer(Timer::from_seconds(
            1.,
            TimerMode::Repeating,
        )));
        app.add_systems(
            Update,
            transport_belt_tick
                .run_if(in_state(AppState::Running))
                .before(BeltItemRenderSet),
        );
        app.add_systems(
            Update,
            create_transport_belt_sprites
                .run_if(in_state(AppState::Running))
                .in_set(BeltItemRenderSet),
        );
    }
}

#[cfg(test)]
mod test {

    use bevy_rapier2d::prelude::{Collider, NoUserData, RapierPhysicsPlugin};

    use crate::discrete_rotation::SideCount;

    use super::*;

    #[test]
    fn transport_belt_rotate_right() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<Scene>();
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
        let dropoff_point = app.world.spawn(TransformBundle::default()).id();

        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        let mut belt = TransportBelt::new(dropoff_point);
        belt.add(0, Item::new("Coal"));

        let belt_entity = app
            .world
            .spawn((
                TransformBundle::default(),
                belt,
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        app.add_systems(Update, transport_belt_tick);

        app.update();

        assert_eq!(
            app.world.get::<TransportBelt>(belt_entity).unwrap().slots,
            vec![None, Some(Item::new("Coal")), None]
        );

        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        app.update();

        assert_eq!(
            app.world.get::<TransportBelt>(belt_entity).unwrap().slots,
            vec![None, None, Some(Item::new("Coal"))]
        );
    }

    #[test]
    fn transport_belt_rotate_right_first_two_slots() {
        let mut app = App::new();

        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<Scene>();
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));
        let dropoff_point = app.world.spawn(TransformBundle::default()).id();
        let mut belt = TransportBelt::new(dropoff_point);
        belt.add(0, Item::new("Coal"));
        belt.add(1, Item::new("Iron ore"));

        let belt_entity = app
            .world
            .spawn((
                TransformBundle::default(),
                belt,
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        app.add_systems(Update, transport_belt_tick);

        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        app.update();

        assert_eq!(
            app.world.get::<TransportBelt>(belt_entity).unwrap().slots,
            vec![None, Some(Item::new("Coal")), Some(Item::new("Iron ore"))]
        );
    }

    #[test]
    fn transport_belt_shift() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<Scene>();
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));

        let dropoff_point = app.world.spawn(TransformBundle::default()).id();
        let mut belt = TransportBelt::new(dropoff_point);
        belt.add(0, Item::new("Coal"));
        belt.add(2, Item::new("Iron ore"));

        let belt_entity = app
            .world
            .spawn((
                TransformBundle::default(),
                belt,
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        app.add_systems(Update, transport_belt_tick);

        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        app.update();

        assert_eq!(
            app.world.get::<TransportBelt>(belt_entity).unwrap().slots,
            vec![None, Some(Item::new("Coal")), Some(Item::new("Iron ore"))]
        );
    }

    #[test]
    fn transport_belt_transfer_to_next_belt() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .init_asset::<Mesh>()
            .init_asset::<Scene>()
            .add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0));

        let dropoff_point = app
            .world
            .spawn((
                Dropoff,
                TransformBundle::from_transform(Transform::from_translation(Vec3::new(1., 0., 0.))),
            ))
            .id();
        let mut belt_a = TransportBelt::new(dropoff_point);
        belt_a.add(1, Item::new("Coal"));
        belt_a.add(2, Item::new("Iron ore"));

        let belt_a_entity = app
            .world
            .spawn((
                belt_a,
                Collider::cuboid(0.5, 0.5),
                TransformBundle::from_transform(Transform::from_translation(Vec3::new(0., 0., 0.))),
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        let belt_b = TransportBelt::new(dropoff_point);
        let belt_b_entity = app
            .world
            .spawn((
                belt_b,
                Collider::cuboid(0.5, 0.5),
                TransformBundle::from_transform(Transform::from_translation(Vec3::new(1., 0., 0.))),
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        app.add_systems(Update, transport_belt_tick);

        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        app.update();

        assert_eq!(
            app.world.get::<TransportBelt>(belt_a_entity).unwrap().slots,
            vec![None, None, Some(Item::new("Coal"))]
        );

        assert_eq!(
            app.world.get::<TransportBelt>(belt_b_entity).unwrap().slots,
            vec![None, Some(Item::new("Iron ore")), None]
        );
    }
}