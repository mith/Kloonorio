use bevy::{
    ecs::{
        query::With,
        system::{Query, Resource},
    },
    gizmos::gizmos::Gizmos,
    math::Vec2,
    render::color::Color,
    transform::components::GlobalTransform,
};

use super::{Chunk, HoveredTile, CHUNK_SIZE};

#[derive(Resource, Default)]
pub struct TerrainDebug;

pub fn chunk_gizmos(mut gizmos: Gizmos, chunk_query: Query<&GlobalTransform, With<Chunk>>) {
    for transform in chunk_query.iter() {
        let half_width = CHUNK_SIZE.x as f32 / 2.0;
        let half_height = CHUNK_SIZE.y as f32 / 2.0;
        let center = transform.translation().truncate();

        // Draw chunk borders
        gizmos.line_2d(
            center + Vec2::new(-half_width, -half_height),
            center + Vec2::new(half_width, -half_height),
            Color::WHITE,
        );
        gizmos.line_2d(
            center + Vec2::new(half_width, -half_height),
            center + Vec2::new(half_width, half_height),
            Color::WHITE,
        );
        gizmos.line_2d(
            center + Vec2::new(half_width, half_height),
            center + Vec2::new(-half_width, half_height),
            Color::WHITE,
        );
        gizmos.line_2d(
            center + Vec2::new(-half_width, half_height),
            center + Vec2::new(-half_width, -half_height),
            Color::WHITE,
        );

        gizmos.rect_2d(center, 0., Vec2::ONE, Color::WHITE);
    }
}

pub fn hovered_tile_gizmo(mut gizmos: Gizmos, hovered_tile_query: Query<&HoveredTile>) {
    for hovered_tile in &hovered_tile_query {
        let tile_center = hovered_tile.tile_center;
        gizmos.rect_2d(tile_center, 0., Vec2::ONE, Color::YELLOW);
    }
}
