use std::collections::VecDeque;

use bevy::{prelude::*, utils::HashSet};

use crate::{discrete_rotation::DiscreteRotation, inventory::Stack, item::Item, types::AppState};

// TODO: Right now there's a fixed 3 slots for simplicity, but it might be interesting to merge
// adjacent belts by pre- or appending the slots from newly constructed belts instead of
// creating separate entities

#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransportBeltSet;

pub struct TransportBeltPlugin;

impl Plugin for TransportBeltPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TransportBelt>()
            .insert_resource(TransportBeltTimer(Timer::from_seconds(
                1.,
                TimerMode::Repeating,
            )))
            .add_systems(
                FixedUpdate,
                (
                    clear_lanes,
                    apply_deferred,
                    construct_lanes,
                    apply_deferred,
                    transport_belt_tick,
                )
                    .chain()
                    .in_set(TransportBeltSet)
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Component, Reflect)]
pub struct TransportBelt {
    slots: VecDeque<Option<Item>>,
}

impl Default for TransportBelt {
    fn default() -> Self {
        Self {
            slots: VecDeque::from(vec![None, None, None]),
        }
    }
}

impl TransportBelt {
    pub fn can_add(&self, slot: usize) -> bool {
        self.slots[slot].is_none()
    }

    /// Add a stack to the belt at the given slot. Returns true if the stack was added
    pub fn add(&mut self, slot: usize, item: Item) -> bool {
        if self.can_add(slot) {
            self.slots[slot] = Some(item);
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

#[derive(Component, Reflect)]
pub struct NextBelt(pub Entity);

#[derive(Component, Reflect)]
pub struct PreviousBelts {
    pub belts: HashSet<Entity>,
}

#[derive(Component, Reflect)]
pub struct Lane {
    belts: Vec<Entity>,
}

fn clear_lanes(mut commands: Commands, lane_query: Query<Entity, With<Lane>>) {
    for lane_entity in &lane_query {
        commands.entity(lane_entity).despawn_recursive();
    }
}

fn construct_lanes(
    mut commands: Commands,
    last_belts_query: Query<Entity, (With<TransportBelt>, Without<NextBelt>)>,
    belts_query: Query<(&TransportBelt, &PreviousBelts)>,
) {
    let mut lanes: Vec<Lane> = Vec::new();
    for belt_entity in &last_belts_query {
        let span = info_span!("Transport belt tick", entity = ?belt_entity);
        let _enter = span.enter();

        // First get a list of all belts in the chain
        let mut belt_lanes = construct_lanes_from_belt_chain(belt_entity, &belts_query);
        lanes.append(&mut belt_lanes);
    }

    // Then create the lane entities
    for lane in lanes {
        commands.spawn(lane);
    }
}

fn construct_lanes_from_belt_chain(
    belt_entity: Entity,
    belts_query: &Query<(&TransportBelt, &PreviousBelts)>,
) -> Vec<Lane> {
    let mut lanes: Vec<Lane> = vec![];
    let mut lane = Lane {
        belts: vec![belt_entity],
    };
    let mut current_belt = belt_entity;
    while let Some((prev_belt, branches)) = get_previous_belt(current_belt, belts_query) {
        lane.belts.push(prev_belt);
        current_belt = prev_belt;
        for branch in branches {
            let mut branch_lanes = construct_lanes_from_belt_chain(branch, belts_query);
            lanes.append(&mut branch_lanes);
        }
    }
    lanes.push(lane);
    lanes
}

/// Return the previous belt in the chain and the set of branches (if any)
fn get_previous_belt(
    belt_entity: Entity,
    belts_query: &Query<(&TransportBelt, &PreviousBelts)>,
) -> Option<(Entity, HashSet<Entity>)> {
    let (_belt, previous_belts) = belts_query.get(belt_entity).ok()?;
    // Find a belt that has an item in the last slot
    let active_prev_belt = previous_belts.belts.iter().find_map(|prev_belt| {
        let prev_belt = *prev_belt;
        let prev_belt_slots = belts_query.get(prev_belt).unwrap().0.slots();
        if prev_belt_slots.last().unwrap().is_some() {
            Some(prev_belt)
        } else {
            None
        }
    });

    // If there is no active previous belt, return the first belt in the chain
    // If there is no previous belt, return None
    let prev_belt = active_prev_belt.or_else(|| previous_belts.belts.iter().next().copied())?;

    let other_belts = previous_belts
        .belts
        .iter()
        .filter(|b| **b != prev_belt)
        .cloned()
        .collect();

    Some((prev_belt, other_belts))
}

#[derive(Resource)]
pub struct TransportBeltTimer(Timer);

pub fn transport_belt_tick(
    lane_query: Query<&Lane>,
    mut belts_query: Query<(&mut TransportBelt, Option<&NextBelt>, &DiscreteRotation)>,
    mut belt_timer: ResMut<TransportBeltTimer>,
    time: Res<Time>,
) {
    if !belt_timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for lane in &lane_query {
        for entity in lane.belts.iter() {
            let span = info_span!("Transport belt tick", entity = ?entity);
            let _enter = span.enter();

            let (last_slot, next_belt_entity, compass_direction) = belts_query
                .get(*entity)
                .map(|b| {
                    (
                        b.0.slots.back().expect("Belt should have slots").clone(),
                        b.1.map(|b| b.0),
                        b.2.compass_direction(),
                    )
                })
                .unwrap();

            // First handle the last slot
            if let Some(product) = last_slot {
                // Transfer to other belt if possible
                let mut transfered = false;
                if let Some((mut belt, _, belt_rotation)) =
                    next_belt_entity.and_then(|e| belts_query.get_mut(e).ok())
                {
                    let stack = Stack::new(product.clone(), 1);
                    let slot = {
                        if belt_rotation.compass_direction() == compass_direction {
                            0
                        } else {
                            1
                        }
                    };
                    transfered = belt.add(slot, stack.item.clone());
                }

                if transfered {
                    debug!("Transfered item to other belt");
                    // Product was transfered to other belt, remove from current belt
                    *belts_query
                        .get_mut(*entity)
                        .as_mut()
                        .unwrap()
                        .0
                        .slots
                        .back_mut()
                        .expect("Belt should have slots") = None;
                    // Rotate the belt 1 slot to the right
                    belts_query
                        .get_mut(*entity)
                        .as_mut()
                        .unwrap()
                        .0
                        .slots
                        .rotate_right(1);
                } else {
                    debug!("Belt full");
                    // Could not transfer to other belt, shift items if there is space
                    // instead of rotating the belt
                    if let Ok((belt, _, _)) = belts_query.get_mut(*entity).as_mut() {
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
                    .get_mut(*entity)
                    .as_mut()
                    .unwrap()
                    .0
                    .slots
                    .rotate_right(1);
            }
        }
    }
}

#[derive(Component, Reflect)]
pub struct BeltItem;

#[cfg(test)]
mod test {

    use bevy::time::TimePlugin;

    use crate::discrete_rotation::{CompassDirection, SideCount};

    use super::*;

    #[test]
    fn transport_belt_rotate_right() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);

        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        let mut belt = TransportBelt::default();
        belt.add(0, Item::new("Coal"));

        let belt_entity = app
            .world
            .spawn((
                TransformBundle::default(),
                belt,
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        app.world.spawn(Lane {
            belts: vec![belt_entity],
        });

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
        app.add_plugins(TimePlugin);

        let mut belt = TransportBelt::default();
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

        app.world.spawn(Lane {
            belts: vec![belt_entity],
        });

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
        app.add_plugins(TimePlugin);
        let mut belt = TransportBelt::default();
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

        app.world.spawn(Lane {
            belts: vec![belt_entity],
        });

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
        app.add_plugins(TimePlugin);

        let belt_b = TransportBelt::default();
        let belt_b_entity = app
            .world
            .spawn((belt_b, DiscreteRotation::new(SideCount::One)))
            .id();

        let mut belt_a = TransportBelt::default();
        belt_a.add(1, Item::new("Coal"));
        belt_a.add(2, Item::new("Iron ore"));

        let belt_a_entity = app
            .world
            .spawn((
                belt_a,
                NextBelt(belt_b_entity),
                DiscreteRotation::new(SideCount::One),
            ))
            .id();

        app.world.spawn(Lane {
            belts: vec![belt_b_entity, belt_a_entity],
        });

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
            vec![Some(Item::new("Iron ore")), None, None]
        );
    }

    #[test]
    fn transport_belt_transfer_to_next_belt_perpendicular() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);

        let belt_b = TransportBelt::default();
        let mut belt_b_rotation = DiscreteRotation::new(SideCount::Four);
        belt_b_rotation.set(CompassDirection::East);
        let belt_b_entity = app.world.spawn((belt_b, belt_b_rotation)).id();

        let mut belt_a = TransportBelt::default();
        belt_a.add(2, Item::new("Coal"));
        let mut belt_a_rotation = DiscreteRotation::new(SideCount::Four);
        belt_a_rotation.set(CompassDirection::North);
        let belt_a_entity = app
            .world
            .spawn((belt_a, NextBelt(belt_b_entity), belt_a_rotation))
            .id();

        app.world.spawn(Lane {
            belts: vec![belt_b_entity, belt_a_entity],
        });

        app.add_systems(Update, transport_belt_tick);
        let timer = TransportBeltTimer(Timer::from_seconds(0., TimerMode::Once));
        app.insert_resource(timer);

        app.update();

        assert_eq!(
            app.world.get::<TransportBelt>(belt_a_entity).unwrap().slots,
            vec![None, None, None]
        );

        assert_eq!(
            app.world.get::<TransportBelt>(belt_b_entity).unwrap().slots,
            vec![None, Some(Item::new("Coal")), None]
        );
    }
}
