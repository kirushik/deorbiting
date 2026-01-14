//! Camera system for the orbital mechanics simulator.
//!
//! Provides zoom, pan, and focus controls for viewing the solar system.

use bevy::{
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    render::camera::ScalingMode,
    window::PrimaryWindow,
};

use crate::input::DragState;

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
#[derive(Resource)]
pub struct CameraFocus {
    /// Target position to animate to (if any).
    pub target_position: Option<Vec2>,
    /// Animation progress (0.0 to 1.0).
    pub progress: f32,
    /// Starting position for animation.
    pub start_position: Vec2,
    /// Animation duration in seconds.
    pub duration: f32,
}

impl Default for CameraFocus {
    fn default() -> Self {
        Self {
            target_position: None,
            progress: 0.0,
            start_position: Vec2::ZERO,
            duration: 0.5,
        }
    }
}

/// Resource for tracking double-clicks.
#[derive(Resource)]
pub struct ClickTracker {
    /// Time of the last click (in seconds since app start).
    pub last_click_time: f64,
    /// Screen position of the last click.
    pub last_click_pos: Vec2,
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self {
            last_click_time: -1.0,
            last_click_pos: Vec2::ZERO,
        }
    }
}

/// Maximum time between clicks to count as double-click (seconds).
const DOUBLE_CLICK_TIME: f64 = 0.3;

/// Maximum distance between clicks to count as double-click (pixels).
const DOUBLE_CLICK_DIST: f32 = 10.0;

/// Animation smoothing factor.
const FOCUS_SMOOTH_FACTOR: f32 = 5.0;

/// Plugin providing camera functionality.
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraState>()
            .init_resource::<CameraFocus>()
            .init_resource::<ClickTracker>()
            .add_systems(Startup, setup_camera)
            .add_systems(Update, (camera_zoom, camera_pan, detect_double_click, animate_focus));
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
    mut contexts: bevy_egui::EguiContexts,
) {
    // Skip if no scroll input
    if mouse_scroll.delta.y == 0.0 {
        return;
    }

    // Skip zoom if egui wants the pointer (e.g., hovering over UI panel)
    if let Some(ctx) = contexts.try_ctx_mut() {
        if ctx.wants_pointer_input() {
            return;
        }
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

/// Handle mouse drag for panning (middle mouse button only).
fn camera_pan(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mut camera_query: Query<(&mut Transform, &Projection), With<MainCamera>>,
    mut contexts: bevy_egui::EguiContexts,
) {
    // Pan with middle mouse button only (left-click is for selection/drag)
    if !mouse_buttons.pressed(MouseButton::Middle) {
        return;
    }

    // Skip pan if egui wants the pointer (e.g., interacting with UI panel)
    if let Some(ctx) = contexts.try_ctx_mut() {
        if ctx.wants_pointer_input() {
            return;
        }
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

/// Detect double-clicks on celestial bodies and initiate focus animation.
fn detect_double_click(
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform, &Transform), With<MainCamera>>,
    mut click_tracker: ResMut<ClickTracker>,
    mut focus: ResMut<CameraFocus>,
    time: Res<Time>,
    drag_state: Res<DragState>,
) {
    // Skip double-click detection during asteroid drag
    if drag_state.dragging.is_some() {
        return;
    }

    // Only trigger on left mouse button just pressed
    if !mouse_buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Ok((camera, camera_global_transform, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let current_time = time.elapsed_secs_f64();

    // Check if this is a double-click
    let time_since_last = current_time - click_tracker.last_click_time;
    let dist_from_last = (cursor_pos - click_tracker.last_click_pos).length();

    if time_since_last < DOUBLE_CLICK_TIME && dist_from_last < DOUBLE_CLICK_DIST {
        // Double-click detected! Convert screen position to world position
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_global_transform, cursor_pos) {
            // Start focus animation
            focus.target_position = Some(world_pos);
            focus.start_position = camera_transform.translation.truncate();
            focus.progress = 0.0;

            info!("Focusing on position: {:?}", world_pos);
        }

        // Reset tracker to prevent triple-click
        click_tracker.last_click_time = -1.0;
    } else {
        // Update click tracker for next potential double-click
        click_tracker.last_click_time = current_time;
        click_tracker.last_click_pos = cursor_pos;
    }
}

/// Animate camera focus towards target position.
fn animate_focus(
    mut camera_query: Query<&mut Transform, With<MainCamera>>,
    mut focus: ResMut<CameraFocus>,
    time: Res<Time>,
) {
    let Some(target) = focus.target_position else {
        return;
    };

    let Ok(mut transform) = camera_query.get_single_mut() else {
        return;
    };

    // Update animation progress
    focus.progress += time.delta_secs() * FOCUS_SMOOTH_FACTOR;

    if focus.progress >= 1.0 {
        // Animation complete
        transform.translation.x = target.x;
        transform.translation.y = target.y;
        focus.target_position = None;
        focus.progress = 0.0;
    } else {
        // Smooth interpolation using ease-out quad
        let t = focus.progress;
        let eased = 1.0 - (1.0 - t) * (1.0 - t);

        let current = focus.start_position.lerp(target, eased);
        transform.translation.x = current.x;
        transform.translation.y = current.y;
    }
}
