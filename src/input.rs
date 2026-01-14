//! Input handling for keyboard shortcuts and mouse dragging.
//!
//! Provides keyboard controls for simulation time, camera zoom, and toggles.
//! Also provides mouse drag support for moving asteroids when paused.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::asteroid::{Asteroid, ResetEvent};
use crate::camera::{MainCamera, RENDER_SCALE, MIN_ZOOM, MAX_ZOOM, ZOOM_SPEED};
use crate::physics::IntegratorStates;
use crate::render::SelectedBody;
use crate::types::{BodyState, SelectableBody, SimulationTime};

/// Resource tracking asteroid drag state.
#[derive(Resource, Default)]
pub struct DragState {
    /// Entity currently being dragged, if any.
    pub dragging: Option<Entity>,
    /// Offset from asteroid center to mouse position at drag start.
    pub drag_offset: DVec2,
}

/// Plugin providing keyboard input handling and mouse drag support.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DragState>()
            .add_systems(Update, (keyboard_shortcuts, handle_asteroid_drag));
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

    // R: reset simulation (time, asteroids, collision state)
    if keys.just_pressed(KeyCode::KeyR) {
        reset_events.send(ResetEvent);
    }
}

/// Handle mouse dragging of asteroids when simulation is paused.
fn handle_asteroid_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut asteroids: Query<&mut BodyState, With<Asteroid>>,
    selected: Res<SelectedBody>,
    sim_time: Res<SimulationTime>,
    mut drag_state: ResMut<DragState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut contexts: EguiContexts,
) {
    // Only allow dragging when paused
    if !sim_time.paused {
        drag_state.dragging = None;
        return;
    }

    // Don't drag if egui wants the pointer
    if let Some(ctx) = contexts.try_ctx_mut() {
        if ctx.wants_pointer_input() {
            return;
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

    // Start drag on mouse down
    if mouse.just_pressed(MouseButton::Left) {
        if let Some(SelectableBody::Asteroid(entity)) = selected.body {
            if let Ok(body_state) = asteroids.get(entity) {
                // Calculate offset from asteroid center
                drag_state.dragging = Some(entity);
                drag_state.drag_offset = body_state.pos - physics_pos;
            }
        }
    }

    // Continue drag
    if mouse.pressed(MouseButton::Left) {
        if let Some(entity) = drag_state.dragging {
            if let Ok(mut body_state) = asteroids.get_mut(entity) {
                // Update position with offset
                body_state.pos = physics_pos + drag_state.drag_offset;
            }
        }
    }

    // End drag on mouse release
    if mouse.just_released(MouseButton::Left) {
        if let Some(entity) = drag_state.dragging {
            if let Ok(mut body_state) = asteroids.get_mut(entity) {
                // Reset velocity to zero
                body_state.vel = DVec2::ZERO;

                // Reinitialize integrator state for this entity
                integrator_states.remove(entity);

                info!("Asteroid moved, velocity reset to zero");
            }
            drag_state.dragging = None;
        }
    }
}
