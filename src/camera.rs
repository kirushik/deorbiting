//! Camera system for the orbital mechanics simulator.
//!
//! Provides zoom, pan, and focus controls for viewing the solar system.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::camera::ScalingMode,
};

/// Render scale: 1 render unit = 1e9 meters (1 Gigameter).
/// This maps the solar system to manageable f32 coordinates.
/// - 1 AU = ~149.6 render units
/// - Full solar system (Neptune) = ~4500 render units
pub const RENDER_SCALE: f64 = 1e-9;

/// Minimum zoom level (closest zoom, planetary close-up).
pub const MIN_ZOOM: f32 = 0.001;

/// Maximum zoom level (furthest zoom, full solar system).
pub const MAX_ZOOM: f32 = 100.0;

/// Default zoom level showing roughly the inner solar system.
pub const DEFAULT_ZOOM: f32 = 1.0;

/// Initial viewport height in render units at scale=1.0.
/// Set to show roughly Mercury-Mars distance initially (~3.3 AU visible).
pub const VIEWPORT_HEIGHT: f32 = 500.0;

/// Zoom speed multiplier for scroll wheel.
pub const ZOOM_SPEED: f32 = 0.1;

/// Pan speed multiplier.
pub const PAN_SPEED: f32 = 1.0;

/// Marker component for the main camera.
#[derive(Component)]
pub struct MainCamera;

/// Resource tracking camera state.
#[derive(Resource)]
pub struct CameraState {
    pub zoom: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self { zoom: DEFAULT_ZOOM }
    }
}

/// Resource for smooth camera focus animation.
#[derive(Resource, Default)]
pub struct CameraFocus {
    pub target_position: Option<Vec2>,
    pub smooth_speed: f32,
}

/// Plugin providing camera functionality.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraState>()
            .init_resource::<CameraFocus>()
            .add_systems(Startup, setup_camera)
            .add_systems(Update, (camera_zoom, camera_pan));
    }
}

/// Spawn the main camera with orthographic projection.
fn setup_camera(mut commands: Commands) {
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
}

/// Handle mouse scroll wheel for zoom.
fn camera_zoom(
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut camera_query: Query<&mut Projection, With<MainCamera>>,
    mut camera_state: ResMut<CameraState>,
) {
    // Skip if no scroll input
    if mouse_scroll.delta.y == 0.0 {
        return;
    }

    let Ok(mut projection) = camera_query.get_single_mut() else {
        return;
    };

    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    // Logarithmic zoom: multiply scale by factor based on scroll direction
    let zoom_factor = 1.0 - mouse_scroll.delta.y * ZOOM_SPEED;
    ortho.scale = (ortho.scale * zoom_factor).clamp(MIN_ZOOM, MAX_ZOOM);
    camera_state.zoom = ortho.scale;
}

/// Handle middle mouse button drag for panning.
fn camera_pan(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut camera_query: Query<(&mut Transform, &Projection), With<MainCamera>>,
) {
    // Pan with middle mouse button
    if !mouse_buttons.pressed(MouseButton::Middle) {
        return;
    }

    let Ok((mut transform, projection)) = camera_query.get_single_mut() else {
        return;
    };

    let Projection::Orthographic(ortho) = projection else {
        return;
    };

    // Convert screen delta to world delta
    // Screen motion is in pixels; scale by current zoom level and viewport
    let scale_factor = ortho.scale * PAN_SPEED;
    let delta = mouse_motion.delta * scale_factor;

    transform.translation.x -= delta.x;
    transform.translation.y += delta.y; // Invert Y for natural feel
}
