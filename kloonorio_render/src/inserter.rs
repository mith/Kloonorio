use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        schedule::{common_conditions::in_state, IntoSystemConfigs},
        system::{Query, Res},
    },
    gizmos::gizmos::Gizmos,
    math::Vec3Swizzles,
    render::{color::Color, view::Visibility},
    transform::components::{GlobalTransform, Transform},
};
use kloonorio_core::{
    structure_components::inserter::{
        inserter_dropoff_location, inserter_pickup_location, Inserter, InserterHand,
        INSERTER_DROPOFF_OFFSET, INSERTER_PICKUP_OFFSET,
    },
    types::AppState,
};
use tracing::info_span;

use crate::{isometric_sprite::IsometricSprite, item_textures::ItemTextures};

pub struct InserterRenderPlugin;

impl Plugin for InserterRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            animate_arm_position.run_if(in_state(AppState::Running)),
        );
    }
}

fn animate_arm_position(
    inserter_query: Query<(&GlobalTransform, &Inserter, &InserterHand)>,
    mut arm_query: Query<(&mut Transform, &mut Visibility, &mut IsometricSprite)>,
    item_textures: Res<ItemTextures>,
    mut gizmos: Gizmos,
) {
    for (inserter_transform, inserter, inserter_hand) in &mut inserter_query.iter() {
        let span = info_span!("Animate arm position", inserter = ?inserter);
        let _enter = span.enter();

        let arm_position = inserter.arm_position();
        let inserter_location = inserter_transform.translation().xy();

        let pickup_location = inserter_pickup_location(inserter_transform);
        let dropoff_location = inserter_dropoff_location(inserter_transform);

        let normalized_arm_position = (arm_position + 1.0) / 2.0;
        let arm_position = pickup_location.lerp(dropoff_location, normalized_arm_position);

        gizmos.circle_2d(arm_position, 0.05, Color::YELLOW);
        gizmos.line_2d(inserter_location, arm_position, Color::YELLOW);

        let (mut hand_transform, mut visibility, mut iso_sprite) =
            arm_query.get_mut(inserter_hand.0).unwrap();
        hand_transform.translation =
            INSERTER_PICKUP_OFFSET.lerp(INSERTER_DROPOFF_OFFSET, normalized_arm_position);

        if let Some(item) = inserter.holding() {
            *visibility = Visibility::Visible;
            iso_sprite.custom_texture_index =
                Some(item_textures.get_texture_index(&item.item).unwrap());
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}
