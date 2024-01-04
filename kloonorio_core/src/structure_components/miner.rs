use bevy::prelude::*;

use crate::{
    drop::DropParams,
    inventory::Stack,
    mineable::Mineable,
    types::{AppState, Powered, Working},
};

pub struct MinerPlugin;

impl Plugin for MinerPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Miner>()
            .add_systems(FixedUpdate, miner_tick.run_if(in_state(AppState::Running)));
    }
}

#[derive(Component, Debug, Reflect)]
pub struct Miner {
    timer: Timer,
    mined_tiles: Vec<Entity>,
    dropoff_tile: Entity,
    current_mineable: Option<Entity>,
}

impl Miner {
    pub fn new(speed: f32, mined_tiles: Vec<Entity>, dropoff_tile: Entity) -> Self {
        Miner {
            timer: Timer::from_seconds(speed, TimerMode::Repeating),
            mined_tiles,
            dropoff_tile,
            current_mineable: None,
        }
    }
}

pub fn miner_tick(
    mut commands: Commands,
    mut miner_query: Query<(Entity, &mut Miner), With<Powered>>,
    time: Res<Time<Fixed>>,
    mineables_query: Query<&Mineable>,
    mut drop_params: DropParams,
) {
    for (miner_entity, mut miner) in miner_query.iter_mut() {
        let span = info_span!("Miner tick", miner = ?miner_entity);
        let _enter = span.enter();

        let current_mineable_entity = {
            if miner.current_mineable.is_none() {
                miner.current_mineable = miner
                    .mined_tiles
                    .iter()
                    .find(|&tile| mineables_query.contains(*tile))
                    .copied();
            }
            miner.current_mineable
        };

        let mut has_dropoff = false;
        let mut has_mineable = false;
        if let Some(current_mineable) =
            current_mineable_entity.and_then(|e| mineables_query.get(e).ok())
        {
            has_mineable = true;
            let stack = Stack::new(current_mineable.0.clone(), 1);
            debug!("Produced {:?}", stack);
            has_dropoff = drop_params.can_drop_stack_at_tile(&stack, miner.dropoff_tile);
            if miner.timer.tick(time.delta()).just_finished() && has_dropoff {
                debug!("Dropping stack");
                drop_params.drop_stack_at_tile(&stack, miner.dropoff_tile);
            }
        }

        if has_mineable && has_dropoff {
            commands.entity(miner_entity).insert(Working);
        } else {
            commands.entity(miner_entity).remove::<Working>();
        }
    }
}
