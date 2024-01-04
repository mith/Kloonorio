use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        component::Component,
        entity::Entity,
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Commands, Query},
    },
    math::{Vec2, Vec3, Vec3Swizzles},
    reflect::Reflect,
    transform::components::GlobalTransform,
};
use kloonorio_core::{structure_components::miner::Miner, types::AppState};
use kloonorio_terrain::TerrainParams;

pub struct MinerBuilderPlugin;

impl Plugin for MinerBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<MinerBuilder>()
            .add_systems(Update, build_miner.run_if(in_state(AppState::Running)));
    }
}

#[derive(Component, Debug, Reflect)]
pub struct MinerBuilder {
    speed: f32,
}

impl MinerBuilder {
    pub fn new(speed: f32) -> Self {
        Self { speed }
    }
}

fn get_miner_covered_tile_positions(pos: Vec2) -> Vec<Vec2> {
    let min_x = pos.x.floor() as i32;
    let min_y = pos.y.floor() as i32;
    let max_x = pos.x.ceil() as i32;
    let max_y = pos.y.ceil() as i32;
    let mut covered_tiles = Vec::new();
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            covered_tiles.push(Vec2::new(x as f32, y as f32));
        }
    }
    covered_tiles
}

fn build_miner(
    mut commands: Commands,
    miner_builder_query: Query<(Entity, &MinerBuilder, &GlobalTransform)>,
    terrain_params: TerrainParams,
) {
    for (miner_entity, miner_builder, transform) in &mut miner_builder_query.iter() {
        let covered_tiles_pos = get_miner_covered_tile_positions(transform.translation().xy());
        let covered_tiles: Vec<Entity> = covered_tiles_pos
            .iter()
            .filter_map(|pos| terrain_params.tile_entity_at_global_pos(*pos))
            .collect();

        let dropoff_tile_location = transform.transform_point(Vec3::new(-0.5, -1.5, 0.)).xy();
        let dropoff_tile_entity = terrain_params
            .tile_entity_at_global_pos(dropoff_tile_location)
            .unwrap();

        let miner = Miner::new(miner_builder.speed, covered_tiles, dropoff_tile_entity);
        commands
            .entity(miner_entity)
            .insert(miner)
            .remove::<MinerBuilder>();
    }
}

#[cfg(test)]
mod test {
    use bevy::math::Vec2;

    #[test]
    fn test_get_miner_covered_tile_positions() {
        let pos = Vec2::new(0.5, 0.5);
        let covered_tiles = super::get_miner_covered_tile_positions(pos);
        assert_eq!(
            covered_tiles,
            vec![
                Vec2::new(0., 0.),
                Vec2::new(0., 1.),
                Vec2::new(1., 0.),
                Vec2::new(1., 1.)
            ]
        );
    }
}
