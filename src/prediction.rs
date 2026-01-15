//! Trajectory prediction for asteroids.
//!
//! This module provides forward simulation of asteroid trajectories,
//! allowing users to see where an asteroid will go based on its current
//! position and velocity.

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::asteroid::Asteroid;
use crate::camera::{CameraState, RENDER_SCALE};
use crate::distortion::distort_position;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::input::DragState;
use crate::physics::compute_acceleration;
use crate::render::z_layers;
use crate::render::SelectedBody;
use crate::types::{BodyState, InputSystemSet, SelectableBody, SimulationTime};
use crate::ui::velocity_handle::VelocityDragState;

/// Plugin providing trajectory prediction functionality.
pub struct PredictionPlugin;

impl Plugin for PredictionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PredictionSettings>()
            .init_resource::<PredictionState>()
            .add_systems(
                Update,
                (
                    track_selection_changes,
                    predict_trajectory.run_if(should_run_prediction),
                    draw_trajectory,
                )
                    .chain()
                    // Must run after input systems so we read updated velocity/position
                    .after(InputSystemSet::PositionDrag),
            );
    }
}

/// Configuration for trajectory prediction.
#[derive(Resource)]
pub struct PredictionSettings {
    /// Maximum number of integration steps per prediction.
    pub max_steps: usize,
    /// Maximum simulation time to predict (seconds).
    pub max_time: f64,
    /// How often to recalculate prediction (frames).
    pub update_interval: u32,
    /// Store a point every N steps (trajectory decimation).
    pub point_interval: usize,
}

impl Default for PredictionSettings {
    fn default() -> Self {
        Self {
            max_steps: 50_000,
            max_time: 5.0 * 365.25 * 24.0 * 3600.0, // 5 years in seconds
            update_interval: 10,
            point_interval: 20, // Store every 20th point (reduced density)
        }
    }
}

/// A single point on a predicted trajectory.
#[derive(Clone, Debug)]
pub struct TrajectoryPoint {
    /// Position in meters from barycenter.
    pub pos: DVec2,
    /// Simulation time in seconds since J2000.
    pub time: f64,
    /// The celestial body whose gravity dominates at this point (None = Sun).
    pub dominant_body: Option<CelestialBodyId>,
}

/// Predicted trajectory path for an asteroid.
#[derive(Component, Default, Clone)]
pub struct TrajectoryPath {
    /// Trajectory points with position, time, and gravitational dominance info.
    pub points: Vec<TrajectoryPoint>,
    /// Whether prediction ended due to collision.
    pub ends_in_collision: bool,
    /// Body that would be hit (if collision predicted).
    pub collision_target: Option<CelestialBodyId>,
}

/// State for prediction system.
///
/// Tracks when predictions need recalculation and allows external
/// systems (like velocity handle) to trigger updates.
#[derive(Resource, Default)]
pub struct PredictionState {
    /// Set when prediction needs recalculation.
    needs_update: bool,
    /// Frame counter for periodic updates.
    frame_counter: u32,
    /// Last selected entity (to detect selection changes).
    last_selected: Option<Entity>,
    /// Last simulation time when prediction was calculated.
    last_sim_time: f64,
}

/// Track selection and time changes to trigger prediction recalculation.
fn track_selection_changes(
    selected: Res<SelectedBody>,
    sim_time: Res<SimulationTime>,
    mut state: ResMut<PredictionState>,
) {
    let current_entity = match selected.body {
        Some(SelectableBody::Asteroid(entity)) => Some(entity),
        _ => None,
    };

    // Check for selection change
    if current_entity != state.last_selected {
        state.needs_update = true;
        state.last_selected = current_entity;
    }

    // Check for significant time change (> 1 day of simulation time)
    const TIME_THRESHOLD: f64 = 86400.0; // 1 day in seconds
    if (sim_time.current - state.last_sim_time).abs() > TIME_THRESHOLD {
        state.needs_update = true;
    }
}

/// Run condition: should we run prediction this frame?
fn should_run_prediction(
    state: Res<PredictionState>,
    settings: Res<PredictionSettings>,
) -> bool {
    state.needs_update || state.frame_counter >= settings.update_interval
}


/// Find the celestial body whose gravity dominates at a given position.
///
/// Compares gravitational acceleration magnitudes (GM/r²) from all bodies.
/// Returns None if the Sun dominates (the default case), or Some(body_id)
/// if a planet or moon's gravity is stronger at that point.
fn find_dominant_body(pos: DVec2, time: f64, ephemeris: &Ephemeris) -> Option<CelestialBodyId> {
    let sources = ephemeris.get_gravity_sources_with_id(time);

    let mut max_acc = 0.0_f64;
    let mut dominant = CelestialBodyId::Sun;

    for (id, body_pos, gm) in sources {
        let delta = body_pos - pos;
        let r_sq = delta.length_squared();

        // Avoid division by zero
        if r_sq < 1.0 {
            return Some(id); // Inside body = that body dominates
        }

        // Gravitational acceleration magnitude: GM/r²
        let acc_mag = gm / r_sq;

        if acc_mag > max_acc {
            max_acc = acc_mag;
            dominant = id;
        }
    }

    // Return None if Sun dominates, Some(id) otherwise
    if dominant == CelestialBodyId::Sun {
        None
    } else {
        Some(dominant)
    }
}

/// Compute trajectory prediction for the selected asteroid.
///
/// Uses Velocity Verlet integrator for all trajectory visualization.
/// This provides consistent, fast results suitable for interactive use.
/// The actual simulation uses IAS15 for high accuracy, but the displayed
/// trajectory preview uses Verlet which is accurate enough for visualization.
fn predict_trajectory(
    mut asteroids: Query<(Entity, &BodyState, &mut TrajectoryPath), With<Asteroid>>,
    selected: Res<SelectedBody>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    settings: Res<PredictionSettings>,
    mut state: ResMut<PredictionState>,
    velocity_drag: Res<VelocityDragState>,
    position_drag: Res<DragState>,
    camera: Res<CameraState>,
) {
    // Increment frame counter
    state.frame_counter += 1;

    // Get selected asteroid entity
    let Some(SelectableBody::Asteroid(selected_entity)) = selected.body else {
        return;
    };

    // Find the selected asteroid
    let Ok((_, body_state, mut trajectory)) = asteroids.get_mut(selected_entity) else {
        return;
    };

    // Skip if velocity is essentially zero
    if body_state.vel.length() < 1.0 {
        trajectory.points.clear();
        state.needs_update = false;
        state.frame_counter = 0;
        state.last_sim_time = sim_time.current;
        return;
    }

    // Clear old trajectory
    trajectory.points.clear();
    trajectory.ends_in_collision = false;
    trajectory.collision_target = None;

    // Store starting point with dominant body calculation
    let start_dominant = find_dominant_body(body_state.pos, sim_time.current, &ephemeris);
    trajectory.points.push(TrajectoryPoint {
        pos: body_state.pos,
        time: sim_time.current,
        dominant_body: start_dominant,
    });

    // Check if we're in interactive drag mode (either position or velocity)
    let is_dragging = velocity_drag.dragging || position_drag.dragging.is_some();

    // Run Velocity Verlet prediction with zoom-dependent timesteps
    predict_with_verlet(
        body_state,
        &ephemeris,
        sim_time.current,
        &settings,
        &mut trajectory,
        is_dragging,
        camera.zoom,
    );

    // Mark prediction as up-to-date
    state.needs_update = false;
    state.frame_counter = 0;
    state.last_sim_time = sim_time.current;
}

/// Predict trajectory using Velocity Verlet integrator.
///
/// Velocity Verlet is O(1) per step and provides sufficient accuracy for
/// trajectory visualization (typically < 0.5% error for circular orbits).
/// The actual simulation uses IAS15 for high precision, but displayed
/// trajectories use this faster method for responsive UI.
///
/// Timestep adapts to zoom level: finer steps when zoomed in (for smooth curves),
/// coarser steps when zoomed out (for performance).
fn predict_with_verlet(
    body_state: &BodyState,
    ephemeris: &Ephemeris,
    start_time: f64,
    settings: &PredictionSettings,
    trajectory: &mut TrajectoryPath,
    is_dragging: bool,
    zoom: f32,
) {
    // Zoom-adaptive timestep: dt scales with sqrt(zoom)
    // - zoom 0.001 (close-up): dt ≈ 23 minutes for smooth curves
    // - zoom 1.0 (medium): dt = 12 hours (baseline)
    // - zoom 100 (far): dt ≈ 5 days (coarse but acceptable)
    let base_dt = 43200.0; // 12 hours in seconds
    let zoom_scale = (zoom as f64).sqrt().clamp(0.03, 4.0);
    
    let (dt, max_steps, point_interval) = if is_dragging {
        // During drag: even coarser steps for responsiveness
        let drag_dt = base_dt * zoom_scale * 4.0; // 4x coarser during drag
        let drag_dt = drag_dt.clamp(3600.0, 86400.0 * 4.0); // 1 hour to 4 days
        (drag_dt, 1000, 2)
    } else {
        // Normal: zoom-adaptive fine steps
        let normal_dt = base_dt * zoom_scale;
        let normal_dt = normal_dt.clamp(900.0, 86400.0 * 2.0); // 15 min to 2 days
        // Adjust max_steps to maintain ~5 year coverage
        let coverage = 5.0 * 365.25 * 86400.0; // 5 years in seconds
        let steps = (coverage / normal_dt) as usize;
        let steps = steps.clamp(1000, 8000); // Keep reasonable bounds
        // Adjust point_interval to keep ~1000 rendered points
        let interval = (steps / 1000).max(1);
        (normal_dt, steps, interval)
    };

    let mut pos = body_state.pos;
    let mut vel = body_state.vel;
    let mut sim_t = start_time;
    let end_t = start_time + settings.max_time;

    for step in 0..max_steps {
        if sim_t >= end_t {
            break;
        }

        // Velocity Verlet integration
        let acc = compute_acceleration(pos, sim_t, ephemeris);
        pos = pos + vel * dt + acc * (0.5 * dt * dt);
        let acc_new = compute_acceleration(pos, sim_t + dt, ephemeris);
        vel = vel + (acc + acc_new) * (0.5 * dt);
        sim_t += dt;

        // Store points at interval with dominant body info
        if step % point_interval == 0 {
            let dominant = find_dominant_body(pos, sim_t, ephemeris);
            trajectory.points.push(TrajectoryPoint {
                pos,
                time: sim_t,
                dominant_body: dominant,
            });
        }

        // Check collision (only when not dragging for performance)
        if !is_dragging {
            if let Some(body_id) = ephemeris.check_collision(pos, sim_t) {
                trajectory.ends_in_collision = true;
                trajectory.collision_target = Some(body_id);
                // Collision point: the collided body dominates
                trajectory.points.push(TrajectoryPoint {
                    pos,
                    time: sim_t,
                    dominant_body: Some(body_id),
                });
                break;
            }
        }

        // Check escape or crash
        const ESCAPE_DISTANCE: f64 = 100.0 * 1.495978707e11; // 100 AU
        const CRASH_DISTANCE: f64 = 1e9; // ~Sun radius
        if pos.length() > ESCAPE_DISTANCE || pos.length() < CRASH_DISTANCE {
            break;
        }
    }
}

/// Draw trajectory using Bevy gizmos.
fn draw_trajectory(
    trajectories: Query<(Entity, &TrajectoryPath), With<Asteroid>>,
    selected: Res<SelectedBody>,
    ephemeris: Res<Ephemeris>,
    _sim_time: Res<SimulationTime>,
    mut gizmos: Gizmos,
) {
    // Get selected asteroid entity
    let Some(SelectableBody::Asteroid(selected_entity)) = selected.body else {
        return;
    };

    // Get trajectory for selected asteroid
    let Ok((_, trajectory)) = trajectories.get(selected_entity) else {
        return;
    };

    // Need at least 2 points to draw lines
    if trajectory.points.len() < 2 {
        return;
    }

    let total_points = trajectory.points.len();
    let mut prev_render_pos: Option<Vec3> = None;

    for (i, point) in trajectory.points.iter().enumerate() {
        // Apply visual distortion
        let distorted_pos = distort_position(point.pos, &ephemeris, point.time);

        // Convert to render coordinates
        let render_pos = Vec3::new(
            (distorted_pos.x * RENDER_SCALE) as f32,
            (distorted_pos.y * RENDER_SCALE) as f32,
            z_layers::TRAJECTORY,
        );

        // Draw line segment from previous point
        if let Some(prev) = prev_render_pos {
            let t_normalized = i as f32 / total_points as f32;
            let color = trajectory_color(
                t_normalized,
                trajectory.ends_in_collision,
                point.dominant_body,
            );
            gizmos.line(prev, render_pos, color);
        }

        prev_render_pos = Some(render_pos);
    }
}

/// Get the characteristic color for a celestial body.
fn body_color(body_id: CelestialBodyId) -> (f32, f32, f32) {
    match body_id {
        CelestialBodyId::Sun => (0.0, 0.85, 1.0),        // Cyan (default trajectory color)
        CelestialBodyId::Mercury => (0.7, 0.7, 0.7),     // Gray
        CelestialBodyId::Venus => (1.0, 0.9, 0.6),       // Yellow
        CelestialBodyId::Earth => (0.3, 0.5, 1.0),       // Blue
        CelestialBodyId::Mars => (1.0, 0.4, 0.3),        // Red
        CelestialBodyId::Jupiter => (1.0, 0.7, 0.3),     // Orange
        CelestialBodyId::Saturn => (0.9, 0.8, 0.5),      // Gold
        CelestialBodyId::Uranus => (0.5, 0.9, 0.8),      // Cyan-green
        CelestialBodyId::Neptune => (0.3, 0.4, 1.0),     // Deep blue
        // Moons inherit parent color (simplified)
        CelestialBodyId::Moon => (0.3, 0.5, 1.0),        // Blue (like Earth)
        CelestialBodyId::Io
        | CelestialBodyId::Europa
        | CelestialBodyId::Ganymede
        | CelestialBodyId::Callisto => (1.0, 0.7, 0.3),  // Orange (like Jupiter)
        CelestialBodyId::Titan => (0.9, 0.8, 0.5),       // Gold (like Saturn)
    }
}

/// Calculate color for trajectory segment based on position along path and dominant body.
fn trajectory_color(
    t_normalized: f32,
    ends_in_collision: bool,
    dominant_body: Option<CelestialBodyId>,
) -> Color {
    // Alpha fades from 1.0 to 0.2 along trajectory
    let alpha = 1.0 - t_normalized * 0.8;

    if ends_in_collision {
        // Collision trajectory: red throughout, intensifying near collision
        // Start orange-red, transition to bright red near collision
        let intensity = 0.6 + t_normalized * 0.4; // 0.6 → 1.0
        let green = 0.3 * (1.0 - t_normalized); // 0.3 → 0.0
        Color::srgba(intensity, green, 0.1, alpha.max(0.5)) // Keep more visible
    } else if let Some(body_id) = dominant_body {
        // Color based on dominant body
        let (r, g, b) = body_color(body_id);
        Color::srgba(r, g, b, alpha)
    } else {
        // Sun-dominated: cyan/teal (default)
        Color::srgba(0.0, 0.85, 1.0, alpha)
    }
}

/// Mark prediction as needing update.
/// Call this when velocity is changed (e.g., from velocity handle drag).
pub fn mark_prediction_dirty(state: &mut PredictionState) {
    state.needs_update = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_color_fades_with_distance() {
        // Sun-dominated (None) trajectory
        let start_color = trajectory_color(0.0, false, None);
        let end_color = trajectory_color(1.0, false, None);

        // Start should be more opaque (higher alpha)
        // Both are cyan colors
        let Color::Srgba(start) = start_color else {
            panic!("Expected Srgba color");
        };
        let Color::Srgba(end) = end_color else {
            panic!("Expected Srgba color");
        };

        assert!(start.alpha > end.alpha, "Color should fade along trajectory");
        assert!(start.alpha > 0.9, "Start should be nearly opaque");
        assert!(end.alpha < 0.3, "End should be mostly transparent");
    }

    #[test]
    fn test_trajectory_color_red_at_collision() {
        let near_collision = trajectory_color(0.95, true, None);
        let normal = trajectory_color(0.95, false, None);

        let Color::Srgba(collision_color) = near_collision else {
            panic!("Expected Srgba color");
        };
        let Color::Srgba(normal_color) = normal else {
            panic!("Expected Srgba color");
        };

        // Collision should be red (high R, low G/B)
        assert!(collision_color.red > 0.8, "Collision should be red");
        assert!(collision_color.green < 0.5, "Collision should have low green");

        // Normal should be cyan (low R, high G/B)
        assert!(normal_color.red < 0.2, "Normal should have low red");
        assert!(normal_color.blue > 0.8, "Normal should be cyan");
    }

    #[test]
    fn test_trajectory_color_by_dominant_body() {
        // Jupiter-dominated segment should be orange
        let jupiter_color = trajectory_color(0.5, false, Some(CelestialBodyId::Jupiter));
        let Color::Srgba(color) = jupiter_color else {
            panic!("Expected Srgba color");
        };

        // Orange: high R, medium G, low B
        assert!(color.red > 0.8, "Jupiter should be orange (high red)");
        assert!(color.green > 0.5 && color.green < 0.9, "Jupiter should be orange (medium green)");
        assert!(color.blue < 0.5, "Jupiter should be orange (low blue)");

        // Earth-dominated segment should be blue
        let earth_color = trajectory_color(0.5, false, Some(CelestialBodyId::Earth));
        let Color::Srgba(e_color) = earth_color else {
            panic!("Expected Srgba color");
        };

        assert!(e_color.blue > 0.8, "Earth should be blue");
        assert!(e_color.red < 0.5, "Earth should have low red");
    }

    #[test]
    fn test_prediction_settings_defaults() {
        let settings = PredictionSettings::default();

        assert_eq!(settings.max_steps, 50_000);
        // 5 years in seconds ≈ 1.577e8
        assert!((settings.max_time - 5.0 * 365.25 * 24.0 * 3600.0).abs() < 1.0);
        assert!(settings.update_interval > 0);
        assert_eq!(settings.point_interval, 20);
    }
}
