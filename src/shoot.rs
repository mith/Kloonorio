use bevy::{
    app::{App, FixedUpdate, Plugin},
    ecs::{
        component::Component,
        entity::Entity,
        query::Without,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query, Res},
    },
    math::Vec3Swizzles,
    reflect::Reflect,
    time::{Fixed, Time, Timer},
    transform::components::GlobalTransform,
};
use kloonorio_core::{health::Health, types::AppState};

pub struct ShootPlugin;

impl Plugin for ShootPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Target>()
            .register_type::<ReloadTimer>()
            .add_systems(
                FixedUpdate,
                (shoot, reload).run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Component)]
pub struct Gun {
    pub range: f32,
    pub damage: u32,
    pub cooldown: f32,
}

#[derive(Component, Reflect)]
pub struct Target(pub Entity);

#[derive(Component, Reflect)]
pub struct ReloadTimer(Timer);

fn shoot(
    mut commands: Commands,
    gun_query: Query<(Entity, &Gun, &Target), Without<ReloadTimer>>,
    global_transform_query: Query<&GlobalTransform>,
    mut health_query: Query<&mut Health>,
) {
    for (gun_entity, gun, Target(target_entity)) in &gun_query {
        let Ok(target_transform) = global_transform_query.get(*target_entity) else {
            commands.entity(gun_entity).remove::<Target>();
            continue;
        };

        let gun_transform = global_transform_query.get(gun_entity).unwrap();

        let distance = gun_transform
            .translation()
            .xy()
            .distance(target_transform.translation().xy());

        if distance <= gun.range {
            commands
                .entity(gun_entity)
                .insert(ReloadTimer(Timer::from_seconds(
                    gun.cooldown,
                    bevy::time::TimerMode::Once,
                )));
            if let Ok(mut health) = health_query.get_mut(*target_entity) {
                health.current -= gun.damage;
            }
        }
    }
}

fn reload(
    mut commands: Commands,
    time: Res<Time<Fixed>>,
    mut reload_timer_query: Query<(Entity, &mut ReloadTimer)>,
) {
    for (gun_entity, mut reload_timer) in &mut reload_timer_query {
        if reload_timer.0.tick(time.delta()).just_finished() {
            commands.entity(gun_entity).remove::<ReloadTimer>();
        }
    }
}
