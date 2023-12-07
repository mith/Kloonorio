use bevy::{
    app::{App, Plugin, Update},
    ecs::{
        event::EventReader,
        system::{Query, Res, Resource},
    },
    input::mouse::MouseWheel,
    render::camera::OrthographicProjection,
};

pub struct PanZoomCameraPlugin;

impl Plugin for PanZoomCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_zoom)
            .insert_resource(CameraSettings {
                zoom_speed: 0.1,
                min_zoom: 0.001,
                max_zoom: 1.,
            });
    }
}

#[derive(Resource)]
pub struct CameraSettings {
    zoom_speed: f32,
    min_zoom: f32,
    max_zoom: f32,
}

fn camera_zoom(
    mut query: Query<&mut OrthographicProjection>,
    mut mouse_wheel_events: EventReader<MouseWheel>,
    camera_settings: Res<CameraSettings>,
) {
    for mut projection in &mut query {
        for event in mouse_wheel_events.read() {
            projection.scale -= projection.scale * event.y * camera_settings.zoom_speed;
            projection.scale = projection
                .scale
                .clamp(camera_settings.min_zoom, camera_settings.max_zoom);
        }
    }
}
