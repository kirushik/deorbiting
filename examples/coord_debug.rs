//! Debug example to test coordinate system behavior.
//!
//! This example sets up a camera like the main app and tests
//! what happens when we convert screen coordinates to world coordinates.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

const VIEWPORT_HEIGHT: f32 = 20.0;
const DEFAULT_ZOOM: f32 = 1.0;

#[derive(Component)]
struct MainCamera;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, track_cursor)
        .run();
}

fn setup(mut commands: Commands) {
    // Same camera setup as main app
    commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: VIEWPORT_HEIGHT,
            },
            scale: DEFAULT_ZOOM,
            near: -10000.0,
            far: 10000.0,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0).looking_at(Vec3::ZERO, Vec3::Y),
        MainCamera,
    ));

    // Spawn a marker at origin
    commands.spawn((
        Mesh3d(bevy::prelude::default()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    info!("Move cursor around. Watch the world_pos values.");
    info!("Moving cursor UP on screen should result in POSITIVE world Y.");
    info!("Moving cursor RIGHT on screen should result in POSITIVE world X.");
}

fn track_cursor(
    window_query: Query<&Window>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
) {
    let Ok(window) = window_query.single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // Only print periodically to avoid spam
    static mut FRAME: u32 = 0;
    unsafe {
        FRAME += 1;
        if FRAME % 60 == 0 {
            info!(
                "Screen: ({:.0}, {:.0}), World: ({:.2}, {:.2})",
                cursor_pos.x, cursor_pos.y, world_pos.x, world_pos.y
            );
        }
    }
}
