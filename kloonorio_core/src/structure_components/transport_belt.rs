use std::collections::VecDeque;

use bevy::prelude::*;

use crate::{discrete_rotation::DiscreteRotation, inventory::Stack, item::Item, types::AppState};

// TODO: Right now there's a fixed 3 slots for simplicity, but it might be interesting to merge
// adjacent belts by pre- or appending the slots from newly constructed belts instead of
// creating separate entities

pub struct TransportBeltPlugin;

impl Plugin for TransportBeltPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<TransportBelt>()
            .insert_resource(TransportBeltTimer(Timer::from_seconds(
                1.,
                TimerMode::Repeating,
            )))
            .add_systems(
                Update,
                transport_belt_tick.run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Component, Reflect)]
pub struct TransportBelt {
    slots: VecDeque<Option<Item>>,
    pub next_belt: Option<Entity>,
}

impl TransportBelt {
    pub fn new(next_belt: Option<Entity>) -> Self {
        TransportBelt {
            slots: VecDeque::from(vec![None, None, None]),
            next_belt,
        }
    }

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

#[derive(Resource)]
pub struct TransportBeltTimer(Timer);

pub fn transport_belt_tick(
    transport_belt_query: Query<Entity, With<TransportBelt>>,
    mut belts_query: Query<(&mut TransportBelt, &DiscreteRotation)>,
    mut belt_timer: ResMut<TransportBeltTimer>,
    time: Res<Time>,
) {
    if !belt_timer.0.tick(time.delta()).just_finished() {
        return;
    }

    for entity in &transport_belt_query {
        let span = info_span!("Transport belt tick", entity = ?entity);
        let _enter = span.enter();

        let (last_slot, next_belt_entity, compass_direction) = belts_query
            .get(entity)
            .map(|b| {
                (
                    b.0.slots.back().expect("Belt should have slots").clone(),
                    b.0.next_belt,
                    b.1.compass_direction(),
                )
            })
            .unwrap();

        // First handle the last slot
        if let Some(product) = last_slot {
            // Transfer to other belt if possible
            let mut transfered = false;
            if let Some((mut belt, belt_rotation)) =
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

        let mut belt = TransportBelt::new(None);
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
        app.add_plugins(TimePlugin);

        let mut belt = TransportBelt::new(None);
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
        app.add_plugins(TimePlugin);
        let mut belt = TransportBelt::new(None);
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
        app.add_plugins(TimePlugin);

        let belt_b = TransportBelt::new(None);
        let belt_b_entity = app
            .world
            .spawn((belt_b, DiscreteRotation::new(SideCount::One)))
            .id();

        let mut belt_a = TransportBelt::new(Some(belt_b_entity));
        belt_a.add(1, Item::new("Coal"));
        belt_a.add(2, Item::new("Iron ore"));

        let belt_a_entity = app
            .world
            .spawn((belt_a, DiscreteRotation::new(SideCount::One)))
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
            vec![Some(Item::new("Iron ore")), None, None]
        );
    }

    #[test]
    fn transport_belt_transfer_to_next_belt_perpendicular() {
        let mut app = App::new();
        app.add_plugins(TimePlugin);

        let belt_b = TransportBelt::new(None);
        let mut belt_b_rotation = DiscreteRotation::new(SideCount::Four);
        belt_b_rotation.set(CompassDirection::East);
        let belt_b_entity = app.world.spawn((belt_b, belt_b_rotation)).id();

        let mut belt_a = TransportBelt::new(Some(belt_b_entity));
        belt_a.add(2, Item::new("Coal"));
        let mut belt_a_rotation = DiscreteRotation::new(SideCount::Four);
        belt_a_rotation.set(CompassDirection::North);
        let belt_a_entity = app.world.spawn((belt_a, belt_a_rotation)).id();

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
