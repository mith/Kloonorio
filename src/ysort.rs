use bevy::{
    app::{App, Plugin, Update},
    ecs::{component::Component, system::Query},
    reflect::Reflect,
    transform::components::Transform,
};

pub struct YSortPlugin;

impl Plugin for YSortPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<YSort>().add_systems(Update, y_sort);
    }
}

#[derive(Component, Debug, Clone, Copy, PartialEq, PartialOrd, Reflect)]
pub struct YSort {
    pub base_layer: f32,
}

fn y_sort(mut q: Query<(&mut Transform, &YSort)>) {
    for (mut tf, ysort) in q.iter_mut() {
        tf.translation.z =
            ysort.base_layer - (1.0f32 / (1.0f32 + (2.0f32.powf(-0.01 * tf.translation.y))));
    }
}
