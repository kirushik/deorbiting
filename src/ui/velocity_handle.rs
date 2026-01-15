//! Velocity handle for interactive velocity editing.
//!
//! Provides a draggable arrow representing an asteroid's velocity vector.
//! When the asteroid is selected and the simulation is paused, users can
//! drag the arrow tip to change the velocity direction and magnitude.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::asteroid::Asteroid;
use crate::camera::MainCamera;
use crate::physics::IntegratorStates;
use crate::prediction::PredictionState;
use crate::render::z_layers;
use crate::render::SelectedBody;
use crate::types::{BodyState, InputSystemSet, SelectableBody, SimulationTime};

/// Plugin for velocity handle interaction.
pub struct VelocityHandlePlugin;

impl Plugin for VelocityHandlePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VelocityDragState>().add_systems(
            Update,
            (
                handle_velocity_drag.in_set(InputSystemSet::VelocityDrag),
                draw_velocity_handle,
            ),
        );
    }
}

/// State for velocity handle dragging.
#[derive(Resource, Default)]
pub struct VelocityDragState {
    /// Currently dragging the velocity handle.
    pub dragging: bool,
    /// Entity being dragged (the asteroid).
    pub target: Option<Entity>,
    /// Initial velocity when drag started.
    pub initial_velocity: DVec2,
    /// Initial cursor position when drag started (render coords).
    pub initial_cursor: Vec2,
}

/// Convert velocity magnitude to arrow length using square root scale.
///
/// Square root scaling provides good resolution in the 1-50 km/s range
/// while still supporting higher velocities. This is ideal for asteroid
/// mission planning where typical velocities are 5-30 km/s.
///
/// Scale mapping examples:
/// - 1 km/s → 4.5 units
/// - 5 km/s → 8.8 units
/// - 10 km/s → 12.1 units
/// - 30 km/s (Earth orbit) → 20.2 units
/// - 50 km/s → 25.7 units
fn velocity_to_arrow_length(vel_magnitude: f64) -> f32 {
    const MIN_LENGTH: f32 = 1.0;  // Base length for zero velocity
    const MAX_LENGTH: f32 = 30.0; // Maximum arrow length
    const SCALE_FACTOR: f32 = 3.5; // Multiplier for sqrt(v_km/s)

    if vel_magnitude < 1.0 {
        // 1 m/s threshold - practically zero for orbital mechanics
        return MIN_LENGTH;
    }

    // Square root scale: good resolution at low velocities, gradual at high
    let vel_km_s = (vel_magnitude / 1000.0) as f32;
    let length = MIN_LENGTH + vel_km_s.sqrt() * SCALE_FACTOR;
    length.clamp(MIN_LENGTH, MAX_LENGTH)
}

/// Convert arrow length back to velocity magnitude.
fn arrow_length_to_velocity(length: f32) -> f64 {
    const MIN_LENGTH: f32 = 1.0;
    const SCALE_FACTOR: f32 = 3.5;

    if length <= MIN_LENGTH {
        return 0.0;
    }

    // Inverse of sqrt scale: v = ((L - MIN) / SCALE)^2 * 1000
    let sqrt_v_km_s = (length - MIN_LENGTH) / SCALE_FACTOR;
    (sqrt_v_km_s * sqrt_v_km_s) as f64 * 1000.0
}

/// Handle mouse interaction with velocity handle.
#[allow(clippy::too_many_arguments)]
fn handle_velocity_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut asteroids: Query<(&Transform, &mut BodyState), With<Asteroid>>,
    selected: Res<SelectedBody>,
    sim_time: Res<SimulationTime>,
    mut drag_state: ResMut<VelocityDragState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    mut contexts: EguiContexts,
) {
    // Only allow velocity editing when paused
    if !sim_time.paused {
        drag_state.dragging = false;
        drag_state.target = None;
        return;
    }

    // Don't interact if egui wants the pointer
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

    // Get selected asteroid
    let Some(SelectableBody::Asteroid(selected_entity)) = selected.body else {
        drag_state.dragging = false;
        drag_state.target = None;
        return;
    };

    let Ok((transform, mut body_state)) = asteroids.get_mut(selected_entity) else {
        return;
    };

    let asteroid_render_pos = transform.translation.truncate();

    // Start drag on mouse down near arrow tip
    if mouse.just_pressed(MouseButton::Left) && !drag_state.dragging {
        // Calculate arrow tip position (same logic as draw_velocity_handle)
        let vel_magnitude = body_state.vel.length();
        let arrow_length = velocity_to_arrow_length(vel_magnitude);
        let direction = if vel_magnitude < 1.0 {
            Vec2::X // Default: point right (same as in draw)
        } else {
            let vel_dir = body_state.vel.normalize_or_zero();
            Vec2::new(vel_dir.x as f32, vel_dir.y as f32)
        };
        let tip_pos = asteroid_render_pos + direction * arrow_length;

        // Check if click is near arrow tip
        let click_radius = 3.0; // Generous click area
        if (world_pos - tip_pos).length() < click_radius {
            drag_state.dragging = true;
            drag_state.target = Some(selected_entity);
            drag_state.initial_velocity = body_state.vel;
            drag_state.initial_cursor = world_pos;
        }
    }

    // Continue drag
    if drag_state.dragging && mouse.pressed(MouseButton::Left) {
        if drag_state.target == Some(selected_entity) {
            // Vector from asteroid to cursor
            let delta = world_pos - asteroid_render_pos;
            let length = delta.length();

            let old_vel = body_state.vel;
            if length > 0.5 {
                // Convert render delta to velocity
                let direction = delta.normalize();
                let new_vel_magnitude = arrow_length_to_velocity(length);

                body_state.vel = DVec2::new(
                    direction.x as f64 * new_vel_magnitude,
                    direction.y as f64 * new_vel_magnitude,
                );
            } else {
                // Very small drag = zero velocity
                body_state.vel = DVec2::ZERO;
            }

            // Real-time preview: update trajectory as velocity changes
            if (body_state.vel - old_vel).length() > 10.0 {
                // Only update if velocity changed significantly (> 10 m/s)
                crate::prediction::mark_prediction_dirty(&mut prediction_state);
            }
        }
    }

    // End drag on mouse release
    if mouse.just_released(MouseButton::Left) && drag_state.dragging {
        if drag_state.target == Some(selected_entity) {
            // Trigger integrator and prediction rebuild
            integrator_states.remove(selected_entity);
            crate::prediction::mark_prediction_dirty(&mut prediction_state);

            info!(
                "Velocity updated to {:.2} km/s",
                body_state.vel.length() / 1000.0
            );
        }
        drag_state.dragging = false;
        drag_state.target = None;
    }
}

/// Draw velocity handle arrow using gizmos.
fn draw_velocity_handle(
    asteroids: Query<(Entity, &Transform, &BodyState), With<Asteroid>>,
    selected: Res<SelectedBody>,
    sim_time: Res<SimulationTime>,
    drag_state: Res<VelocityDragState>,
    mut gizmos: Gizmos,
) {
    // Only show velocity handle when paused
    if !sim_time.paused {
        return;
    }

    // Get selected asteroid
    let Some(SelectableBody::Asteroid(selected_entity)) = selected.body else {
        return;
    };

    let Ok((_, transform, body_state)) = asteroids.get(selected_entity) else {
        return;
    };

    let vel_magnitude = body_state.vel.length();

    // Get asteroid render position (already distorted by sync system)
    let base = Vec3::new(
        transform.translation.x,
        transform.translation.y,
        z_layers::UI_HANDLES,
    );

    // Calculate arrow geometry
    // Use default direction (right) for zero/very small velocity
    let arrow_length = velocity_to_arrow_length(vel_magnitude);
    let direction = if vel_magnitude < 1.0 {
        Vec2::X // Default: point right
    } else {
        let vel_dir = body_state.vel.normalize_or_zero();
        Vec2::new(vel_dir.x as f32, vel_dir.y as f32)
    };

    // Choose color based on drag state and velocity
    let color = if drag_state.dragging && drag_state.target == Some(selected_entity) {
        Color::srgba(1.0, 1.0, 0.0, 1.0) // Bright yellow when dragging
    } else if vel_magnitude < 100.0 {
        Color::srgba(0.5, 0.5, 0.5, 0.7) // Gray for zero/near-zero velocity
    } else {
        Color::srgba(0.0, 1.0, 0.5, 0.9) // Green normally
    };

    // Draw arrow
    draw_arrow(&mut gizmos, base, direction, arrow_length, color);
}

/// Draw an arrow with arrowhead.
fn draw_arrow(gizmos: &mut Gizmos, base: Vec3, direction: Vec2, length: f32, color: Color) {
    let tip = base + Vec3::new(direction.x * length, direction.y * length, 0.0);

    // Main arrow line
    gizmos.line(base, tip, color);

    // Arrowhead (two lines at ~30 degrees)
    let head_size = (length * 0.15).max(1.0);
    let angle = direction.y.atan2(direction.x);

    for offset in [-0.5_f32, 0.5_f32] {
        let head_angle = angle + std::f32::consts::PI + offset;
        let head_end = tip + Vec3::new(
            head_angle.cos() * head_size,
            head_angle.sin() * head_size,
            0.0,
        );
        gizmos.line(tip, head_end, color);
    }

    // Draw a small circle at the tip for easier clicking
    let tip_radius = 0.8;
    let segments = 12;
    for i in 0..segments {
        let t0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let t1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p0 = tip + Vec3::new(tip_radius * t0.cos(), tip_radius * t0.sin(), 0.0);
        let p1 = tip + Vec3::new(tip_radius * t1.cos(), tip_radius * t1.sin(), 0.0);

        gizmos.line(p0, p1, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_velocity_arrow_length_minimum() {
        // Very low velocity should give minimum length
        // Threshold is 1 m/s - below that returns MIN_LENGTH
        assert!(velocity_to_arrow_length(0.0) <= 1.1);
        assert!(velocity_to_arrow_length(0.5) <= 1.1);
    }

    #[test]
    fn test_velocity_arrow_length_increases() {
        // Higher velocity = longer arrow
        let len_1kms = velocity_to_arrow_length(1_000.0);
        let len_10kms = velocity_to_arrow_length(10_000.0);
        let len_30kms = velocity_to_arrow_length(30_000.0);

        assert!(len_10kms > len_1kms, "10 km/s should be longer than 1 km/s");
        assert!(len_30kms > len_10kms, "30 km/s should be longer than 10 km/s");
    }

    #[test]
    fn test_velocity_arrow_length_maximum() {
        // Very high velocity should be clamped to maximum
        let len = velocity_to_arrow_length(1_000_000.0);
        assert!(len <= 30.0, "Arrow length should not exceed maximum");
    }

    #[test]
    fn test_arrow_length_roundtrip() {
        // Converting velocity -> length -> velocity should be approximately reversible
        let original_vel = 15_000.0; // 15 km/s
        let length = velocity_to_arrow_length(original_vel);
        let recovered_vel = arrow_length_to_velocity(length);

        let error = (recovered_vel - original_vel).abs() / original_vel;
        assert!(error < 0.01, "Roundtrip error should be < 1%, got {}%", error * 100.0);
    }
}
