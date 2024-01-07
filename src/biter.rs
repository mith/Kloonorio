use bevy::{
    app::{App, FixedUpdate, Plugin},
    asset::Assets,
    core::Name,
    ecs::{
        component::Component,
        entity::Entity,
        query::{With, Without},
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query, Res, ResMut, Resource, SystemParam},
    },
    hierarchy::BuildChildren,
    math::{Vec2, Vec3Swizzles},
    prelude::default,
    reflect::Reflect,
    render::{
        color::Color,
        mesh::{shape, Mesh},
    },
    sprite::{ColorMaterial, MaterialMesh2dBundle},
    time::{Fixed, Time, Timer, TimerMode},
    transform::{
        components::{GlobalTransform, Transform},
        TransformBundle,
    },
};
use bevy_rapier2d::{control::KinematicCharacterController, geometry::Collider};
use kloonorio_core::{health::Health, player::Player, types::AppState};
use kloonorio_terrain::Chunk;
use rand::{seq::IteratorRandom, RngCore, SeedableRng};
use rand_xoshiro::Xoshiro256StarStar;
use tracing::info;

use crate::{
    shoot::{Gun, Target},
    ysort::YSort,
};

pub struct BiterPlugin;

impl Plugin for BiterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnTimer>()
            .init_resource::<SpawnRng>()
            .add_systems(
                FixedUpdate,
                (spawn_biter_packs, move_to_player, attack_player)
                    .run_if(in_state(AppState::Running)),
            );
    }
}

#[derive(Component)]
pub struct Biter;

#[derive(Resource, Reflect)]
struct SpawnTimer(Timer);

impl Default for SpawnTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(120., TimerMode::Repeating))
    }
}

#[derive(Resource)]
struct SpawnRng(Xoshiro256StarStar);

impl Default for SpawnRng {
    fn default() -> Self {
        Self(Xoshiro256StarStar::seed_from_u64(12345678))
    }
}

fn spawn_biter_packs(
    mut spawn_param: BiterSpawnParam,
    time: Res<Time<Fixed>>,
    mut timer: ResMut<SpawnTimer>,
    mut rng: ResMut<SpawnRng>,
    chunks_query: Query<&Chunk>,
    player_query: Query<&GlobalTransform, With<Player>>,
) {
    if timer.0.tick(time.delta()).just_finished() {
        // Pick a random chunk to spawn in
        // The chunk must be at least 25 chunks away from the center chunk (0, 0) and the player
        let eligible_chunks = chunks_query.iter().filter(|chunk| {
            let chunk_position = chunk.position().as_vec2() * 3.;
            let player_position = player_query.single().translation().xy();

            chunk_position.distance(player_position) > 25.
                && chunk_position.distance(Vec2::ZERO) > 25.
        });

        if let Some(chunk) = eligible_chunks.choose(&mut rng.0) {
            let chunk_position = chunk.position().as_vec2() * 3.;

            let amount = rng.0.next_u32() % 5 + 1;
            info!("Spawning {} biters at {:?}", amount, chunk_position);

            for _ in 0..amount {
                let spawn_position = chunk_position
                    + Vec2::new(
                        ((rng.0.next_u32() % 9) as i32 - 4) as f32,
                        ((rng.0.next_u32() % 9) as i32 - 4) as f32,
                    );
                spawn_biter(&mut spawn_param, spawn_position);
            }
        }
    }
}

#[derive(SystemParam)]
struct BiterSpawnParam<'w, 's> {
    commands: Commands<'w, 's>,
    meshes: ResMut<'w, Assets<Mesh>>,
    materials: ResMut<'w, Assets<ColorMaterial>>,
}

fn spawn_biter(spawn_param: &mut BiterSpawnParam, position: Vec2) {
    let BiterSpawnParam {
        commands,
        meshes,
        materials,
    } = spawn_param;
    commands
        .spawn((
            Name::new("Biter"),
            YSort { base_layer: 1.0 },
            TransformBundle::from_transform(Transform::from_translation(position.extend(1.))),
            Biter,
            Health::new(100),
            Gun {
                range: 1.,
                damage: 5,
                cooldown: 1.,
            },
            Collider::ball(0.36),
            KinematicCharacterController::default(),
        ))
        .with_children(|parent| {
            parent.spawn((
                Name::new("Biter sprite"),
                MaterialMesh2dBundle {
                    mesh: meshes.add(shape::Circle::new(0.36).into()).into(),
                    material: materials.add(ColorMaterial::from(Color::RED)),
                    ..default()
                },
            ));
        });
}

fn move_to_player(
    player_query: Query<&GlobalTransform, With<Player>>,
    mut biter_query: Query<(&mut KinematicCharacterController, &GlobalTransform), With<Biter>>,
) {
    let player_position = player_query.single().translation().xy();

    for (mut biter_controller, biter_transform) in biter_query.iter_mut() {
        let direction_to_player = player_position - biter_transform.translation().xy();

        if direction_to_player.length() > 0.1 {
            biter_controller.translation = Some(direction_to_player.normalize() * 0.1);
        }
    }
}

fn attack_player(
    mut commands: Commands,
    player_query: Query<Entity, With<Player>>,
    biter_query: Query<Entity, (With<Biter>, Without<Target>)>,
) {
    for biter in biter_query.iter() {
        let player = player_query.iter().next().unwrap();
        commands.entity(biter).insert(Target(player));
    }
}

#[cfg(test)]
mod test {
    use rand::RngCore;

    #[test]
    fn spawn_rng() {
        let mut spawn_rng = super::SpawnRng::default();

        let a = spawn_rng.0.next_u64();
        let b = spawn_rng.0.next_u64();

        assert_ne!(a, b);
    }
}
