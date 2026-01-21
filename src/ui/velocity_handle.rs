//! Velocity handle for interactive velocity editing.
//!
//! Provides draggable arrows representing asteroid velocity vectors.
//! Arrows are ALWAYS visible for ALL asteroids:
//! - Selected asteroid: bright green arrow
//! - Other asteroids: dim gray arrow
//!
//! Dragging any arrow auto-pauses the simulation and allows velocity editing.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::asteroid::Asteroid;
use crate::camera::MainCamera;
use crate::collision::CollisionState;
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
                draw_velocity_handles,
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
    /// Whether we auto-paused when starting drag.
    pub auto_paused: bool,
}

/// Convert velocity magnitude to arrow length using square root scale.
///
/// Square root scaling provides good resolution in the 1-50 km/s range
/// while still supporting higher velocities. This is ideal for asteroid
/// mission planning where typical velocities are 5-30 km/s.
fn velocity_to_arrow_length(vel_magnitude: f64) -> f32 {
    const MIN_LENGTH: f32 = 1.0;  // Base length for zero velocity
    const MAX_LENGTH: f32 = 30.0; // Maximum arrow length
    const SCALE_FACTOR: f32 = 3.5; // Multiplier for sqrt(v_km/s)

    if vel_magnitude < 1.0 {
        return MIN_LENGTH;
    }

    let vel_km_s = (vel_magnitude / 1000.0) as f32;
    let length = MIN_LENGTH + vel_km_s.sqrt() * SCALE_FACTOR;
    length.clamp(MIN_LENGTH, MAX_LENGTH)
}

/// Convert arrow length back to velocity magnitude.
/// Caps at MAX_LENGTH for consistency with velocity_to_arrow_length.
fn arrow_length_to_velocity(length: f32) -> f64 {
    const MIN_LENGTH: f32 = 1.0;
    const MAX_LENGTH: f32 = 30.0; // Must match velocity_to_arrow_length
    const SCALE_FACTOR: f32 = 3.5;

    // Cap input length to match visual display limits
    let capped_length = length.clamp(MIN_LENGTH, MAX_LENGTH);

    if capped_length <= MIN_LENGTH {
        return 0.0;
    }

    let sqrt_v_km_s = (capped_length - MIN_LENGTH) / SCALE_FACTOR;
    (sqrt_v_km_s * sqrt_v_km_s) as f64 * 1000.0
}

/// Handle mouse interaction with velocity handles.
/// Now supports clicking ANY asteroid's arrow, not just selected.
/// Auto-pauses simulation when starting to drag.
#[allow(clippy::too_many_arguments)]
fn handle_velocity_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut asteroids: Query<(Entity, &Transform, &mut BodyState), With<Asteroid>>,
    mut selected: ResMut<SelectedBody>,
    mut sim_time: ResMut<SimulationTime>,
    mut drag_state: ResMut<VelocityDragState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    mut contexts: EguiContexts,
) {
    // Validate target entity still exists (could be despawned via collision)
    if let Some(entity) = drag_state.target
        && !asteroids.contains(entity) {
            drag_state.dragging = false;
            drag_state.target = None;
            drag_state.auto_paused = false;
        }

    // IMPORTANT: Only check egui wants pointer when NOT already dragging.
    // If we're dragging and mouse passes over an egui window, we still need
    // to process drag updates and mouse release.
    if !drag_state.dragging
        && let Some(ctx) = contexts.try_ctx_mut()
            && ctx.wants_pointer_input() {
                return;
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

    // Check hover state for cursor feedback
    let hover_radius = 4.0;
    let mut hovering_arrow = false;

    for (_, transform, body_state) in asteroids.iter() {
        let asteroid_render_pos = transform.translation.truncate();
        let vel_magnitude = body_state.vel.length();
        let arrow_length = velocity_to_arrow_length(vel_magnitude);
        let direction = if vel_magnitude < 1.0 {
            Vec2::X
        } else {
            let vel_dir = body_state.vel.normalize_or_zero();
            Vec2::new(vel_dir.x as f32, vel_dir.y as f32)
        };
        let tip_pos = asteroid_render_pos + direction * arrow_length;

        if (world_pos - tip_pos).length() < hover_radius {
            hovering_arrow = true;
            break;
        }
    }

    // Set cursor based on state
    if let Some(ctx) = contexts.try_ctx_mut() {
        if drag_state.dragging {
            ctx.set_cursor_icon(bevy_egui::egui::CursorIcon::Grabbing);
        } else if hovering_arrow {
            ctx.set_cursor_icon(bevy_egui::egui::CursorIcon::Grab);
        }
    }

    // Start drag on mouse down near ANY asteroid's arrow tip
    if mouse.just_pressed(MouseButton::Left) && !drag_state.dragging {
        let click_radius = 3.5; // Generous click area

        // Find the closest arrow tip that was clicked
        let mut closest_entity: Option<Entity> = None;
        let mut closest_dist = click_radius;

        for (entity, transform, body_state) in asteroids.iter() {
            let asteroid_render_pos = transform.translation.truncate();
            let vel_magnitude = body_state.vel.length();
            let arrow_length = velocity_to_arrow_length(vel_magnitude);
            let direction = if vel_magnitude < 1.0 {
                Vec2::X
            } else {
                let vel_dir = body_state.vel.normalize_or_zero();
                Vec2::new(vel_dir.x as f32, vel_dir.y as f32)
            };
            let tip_pos = asteroid_render_pos + direction * arrow_length;

            let dist = (world_pos - tip_pos).length();
            if dist < closest_dist {
                closest_dist = dist;
                closest_entity = Some(entity);
            }
        }

        if let Some(entity) = closest_entity {
            // Auto-pause if simulation is running
            let was_paused = sim_time.paused;
            if !was_paused {
                sim_time.paused = true;
                drag_state.auto_paused = true;
            } else {
                drag_state.auto_paused = false;
            }

            // Select this asteroid
            selected.body = Some(SelectableBody::Asteroid(entity));

            // Get initial velocity
            if let Ok((_, _, body_state)) = asteroids.get(entity) {
                drag_state.dragging = true;
                drag_state.target = Some(entity);
                drag_state.initial_velocity = body_state.vel;
                drag_state.initial_cursor = world_pos;
            }
        }
    }

    // Continue drag
    if drag_state.dragging && mouse.pressed(MouseButton::Left)
        && let Some(target_entity) = drag_state.target
            && let Ok((_, transform, mut body_state)) = asteroids.get_mut(target_entity) {
                let asteroid_render_pos = transform.translation.truncate();

                // Vector from asteroid to cursor
                let delta = world_pos - asteroid_render_pos;
                let length = delta.length();

                let old_vel = body_state.vel;
                if length > 0.5 {
                    let direction = delta.normalize();
                    let new_vel_magnitude = arrow_length_to_velocity(length);

                    body_state.vel = DVec2::new(
                        direction.x as f64 * new_vel_magnitude,
                        direction.y as f64 * new_vel_magnitude,
                    );
                } else {
                    body_state.vel = DVec2::ZERO;
                }

                // Real-time preview
                if (body_state.vel - old_vel).length() > 10.0 {
                    crate::prediction::mark_prediction_dirty(&mut prediction_state);
                }
            }

    // End drag on mouse release
    if mouse.just_released(MouseButton::Left) && drag_state.dragging {
        if let Some(target_entity) = drag_state.target {
            // Only update if entity still exists
            if asteroids.contains(target_entity) {
                integrator_states.remove(target_entity);
                crate::prediction::mark_prediction_dirty(&mut prediction_state);

                if let Ok((_, _, body_state)) = asteroids.get(target_entity) {
                    info!(
                        "Velocity updated to {:.2} km/s",
                        body_state.vel.length() / 1000.0
                    );
                }
            }
        }
        drag_state.dragging = false;
        drag_state.target = None;

        // Restore simulation state: if we auto-paused on drag start, unpause now
        if drag_state.auto_paused {
            sim_time.paused = false;
            drag_state.auto_paused = false;
        }
    }
}

/// Draw velocity arrows for ALL asteroids.
/// Selected asteroid: bright green
/// Other asteroids: dim gray
fn draw_velocity_handles(
    asteroids: Query<(Entity, &Transform, &BodyState), With<Asteroid>>,
    selected: Res<SelectedBody>,
    collision_state: Res<CollisionState>,
    drag_state: Res<VelocityDragState>,
    mut gizmos: Gizmos,
) {
    let selected_entity = if let Some(SelectableBody::Asteroid(e)) = selected.body {
        Some(e)
    } else {
        None
    };

    for (entity, transform, body_state) in asteroids.iter() {
        // Skip colliding entities (in one-frame window before despawn)
        if collision_state.is_colliding(entity) {
            continue;
        }
        let vel_magnitude = body_state.vel.length();

        let base = Vec3::new(
            transform.translation.x,
            transform.translation.y,
            z_layers::UI_HANDLES,
        );

        let arrow_length = velocity_to_arrow_length(vel_magnitude);
        let direction = if vel_magnitude < 1.0 {
            Vec2::X
        } else {
            let vel_dir = body_state.vel.normalize_or_zero();
            Vec2::new(vel_dir.x as f32, vel_dir.y as f32)
        };

        // Choose color based on selection and drag state
        let is_selected = selected_entity == Some(entity);
        let is_dragging = drag_state.dragging && drag_state.target == Some(entity);

        let color = if is_dragging {
            Color::srgba(1.0, 1.0, 0.0, 1.0) // Bright yellow when dragging
        } else if is_selected {
            if vel_magnitude < 100.0 {
                Color::srgba(0.5, 0.5, 0.5, 0.7) // Gray for zero velocity
            } else {
                Color::srgba(0.33, 0.87, 0.53, 0.95) // Bright green for selected
            }
        } else {
            // Dim for non-selected
            Color::srgba(0.4, 0.4, 0.4, 0.4)
        };

        draw_arrow(&mut gizmos, base, direction, arrow_length, color, is_selected || is_dragging);
    }
}

/// Draw an arrow with arrowhead.
fn draw_arrow(gizmos: &mut Gizmos, base: Vec3, direction: Vec2, length: f32, color: Color, show_grip: bool) {
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

    // Draw a small circle at the tip for easier clicking (only for selected/dragging)
    if show_grip {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_velocity_arrow_length_minimum() {
        assert!(velocity_to_arrow_length(0.0) <= 1.1);
        assert!(velocity_to_arrow_length(0.5) <= 1.1);
    }

    #[test]
    fn test_velocity_arrow_length_increases() {
        let len_1kms = velocity_to_arrow_length(1_000.0);
        let len_10kms = velocity_to_arrow_length(10_000.0);
        let len_30kms = velocity_to_arrow_length(30_000.0);

        assert!(len_10kms > len_1kms, "10 km/s should be longer than 1 km/s");
        assert!(len_30kms > len_10kms, "30 km/s should be longer than 10 km/s");
    }

    #[test]
    fn test_velocity_arrow_length_maximum() {
        let len = velocity_to_arrow_length(1_000_000.0);
        assert!(len <= 30.0, "Arrow length should not exceed maximum");
    }

    #[test]
    fn test_arrow_length_roundtrip() {
        let original_vel = 15_000.0;
        let length = velocity_to_arrow_length(original_vel);
        let recovered_vel = arrow_length_to_velocity(length);

        let error = (recovered_vel - original_vel).abs() / original_vel;
        assert!(error < 0.01, "Roundtrip error should be < 1%, got {}%", error * 100.0);
    }
}
