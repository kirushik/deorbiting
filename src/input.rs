//! Input handling for keyboard shortcuts and mouse dragging.
//!
//! Provides keyboard controls for simulation time, camera zoom, and toggles.
//! Mouse drag support for moving asteroids with auto-pause.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::asteroid::{Asteroid, ResetEvent};
use crate::camera::{MainCamera, RENDER_SCALE, MIN_ZOOM, MAX_ZOOM, ZOOM_SPEED};
use crate::physics::IntegratorStates;
use crate::prediction::{mark_prediction_dirty, PredictionState};
use crate::render::SelectedBody;
use crate::types::{BodyState, InputSystemSet, SelectableBody, SimulationTime};
use crate::ui::velocity_handle::VelocityDragState;

/// Resource tracking asteroid drag state.
#[derive(Resource, Default)]
pub struct DragState {
    /// Entity currently being dragged, if any.
    pub dragging: Option<Entity>,
    /// Offset from asteroid center to mouse position at drag start.
    pub drag_offset: DVec2,
    /// Whether we auto-paused when starting drag.
    pub auto_paused: bool,
}

/// Plugin providing keyboard input handling and mouse drag support.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DragState>()
            // Configure system ordering: velocity drag runs before position drag
            .configure_sets(
                Update,
                InputSystemSet::PositionDrag.after(InputSystemSet::VelocityDrag),
            )
            .add_systems(
                Update,
                (
                    keyboard_shortcuts,
                    handle_asteroid_drag.in_set(InputSystemSet::PositionDrag),
                ),
            );
    }
}

/// Handle keyboard shortcuts for simulation control.
fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim_time: ResMut<SimulationTime>,
    mut camera_query: Query<&mut Projection, With<MainCamera>>,
    mut reset_events: EventWriter<ResetEvent>,
) {
    // Space: toggle pause
    if keys.just_pressed(KeyCode::Space) {
        sim_time.paused = !sim_time.paused;
        info!("Simulation {}", if sim_time.paused { "paused" } else { "running" });
    }

    // Handle zoom with keyboard
    let Ok(mut projection) = camera_query.get_single_mut() else {
        return;
    };

    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    // Plus/Equal: zoom in (reduce scale)
    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        let zoom_factor = 1.0 - ZOOM_SPEED;
        ortho.scale = (ortho.scale * zoom_factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    // Minus: zoom out (increase scale)
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        let zoom_factor = 1.0 + ZOOM_SPEED;
        ortho.scale = (ortho.scale * zoom_factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    // Time controls: [ and ] to adjust simulation speed
    if keys.just_pressed(KeyCode::BracketLeft) {
        sim_time.scale = (sim_time.scale * 0.5).max(0.125);
        info!("Time scale: {}x", sim_time.scale);
    }

    if keys.just_pressed(KeyCode::BracketRight) {
        sim_time.scale = (sim_time.scale * 2.0).min(128.0);
        info!("Time scale: {}x", sim_time.scale);
    }

    // Quick time scale selection with number keys
    if keys.just_pressed(KeyCode::Digit1) {
        sim_time.scale = 1.0;
        info!("Time scale: 1x (real-time)");
    }
    if keys.just_pressed(KeyCode::Digit2) {
        sim_time.scale = 10.0;
        info!("Time scale: 10x");
    }
    if keys.just_pressed(KeyCode::Digit3) {
        sim_time.scale = 100.0;
        info!("Time scale: 100x");
    }
    if keys.just_pressed(KeyCode::Digit4) {
        sim_time.scale = 1000.0;
        info!("Time scale: 1000x");
    }

    // R: reset simulation (time, asteroids, collision state)
    if keys.just_pressed(KeyCode::KeyR) {
        reset_events.send(ResetEvent);
    }
}

/// Handle mouse dragging of asteroids (body position).
/// Auto-pauses simulation when starting to drag.
#[allow(clippy::too_many_arguments)]
fn handle_asteroid_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut asteroids: Query<(Entity, &Transform, &mut BodyState), With<Asteroid>>,
    mut selected: ResMut<SelectedBody>,
    mut sim_time: ResMut<SimulationTime>,
    mut drag_state: ResMut<DragState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    velocity_drag_state: Res<VelocityDragState>,
    mut contexts: EguiContexts,
) {
    // Don't start position drag if velocity drag is active
    if velocity_drag_state.dragging {
        return;
    }

    // IMPORTANT: Only check egui wants pointer when NOT already dragging.
    // If we're dragging and mouse passes over an egui window, we still need
    // to process drag updates and mouse release.
    if drag_state.dragging.is_none() {
        if let Some(ctx) = contexts.try_ctx_mut() {
            if ctx.wants_pointer_input() {
                return;
            }
        }
    }

    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        return;
    };

    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        return;
    };

    // Convert screen position to physics position
    let physics_pos = DVec2::new(
        (world_pos.x as f64) / RENDER_SCALE,
        (world_pos.y as f64) / RENDER_SCALE,
    );

    // Start drag on mouse down on asteroid body (not arrow tip)
    if mouse.just_pressed(MouseButton::Left) && drag_state.dragging.is_none() {
        // Check if clicking on any asteroid body
        let click_radius = 3.0; // Render units

        for (entity, transform, body_state) in asteroids.iter() {
            let asteroid_render_pos = transform.translation.truncate();
            let dist = (world_pos - asteroid_render_pos).length();

            if dist < click_radius {
                // Auto-pause if simulation is running
                let was_paused = sim_time.paused;
                if !was_paused {
                    sim_time.paused = true;
                    drag_state.auto_paused = true;
                } else {
                    drag_state.auto_paused = false;
                }

                // Select and start dragging this asteroid
                selected.body = Some(SelectableBody::Asteroid(entity));
                drag_state.dragging = Some(entity);
                drag_state.drag_offset = body_state.pos - physics_pos;
                break;
            }
        }
    }

    // Continue drag
    if mouse.pressed(MouseButton::Left) {
        if let Some(entity) = drag_state.dragging {
            if let Ok((_, _, mut body_state)) = asteroids.get_mut(entity) {
                let old_pos = body_state.pos;
                body_state.pos = physics_pos + drag_state.drag_offset;

                // Real-time preview
                if (body_state.pos - old_pos).length() > 1e6 {
                    mark_prediction_dirty(&mut prediction_state);
                }
            }
        }
    }

    // End drag on mouse release
    if mouse.just_released(MouseButton::Left) {
        if let Some(entity) = drag_state.dragging {
            if asteroids.contains(entity) {
                integrator_states.remove(entity);
                mark_prediction_dirty(&mut prediction_state);
                info!("Asteroid repositioned, velocity preserved");
            }
            drag_state.dragging = None;
            // Note: Keep auto_paused state - user can resume manually with Space
        }
    }
}
