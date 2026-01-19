//! Visualization for continuous deflection spacecraft.
//!
//! Renders:
//! - Transit trajectory (dashed line from Earth to asteroid)
//! - Operating spacecraft icon near asteroid
//! - Thrust direction indicator

use bevy::prelude::*;

use crate::camera::RENDER_SCALE;
use crate::continuous::{ContinuousDeflector, ContinuousDeflectorState, ContinuousPayload};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::types::{BodyState, SimulationTime};
use crate::asteroid::Asteroid;

use super::z_layers;

/// System to draw continuous deflector visualization.
pub fn draw_deflector_trajectories(
    deflectors: Query<&ContinuousDeflector>,
    asteroids: Query<&BodyState, With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    mut gizmos: Gizmos,
) {
    for deflector in deflectors.iter() {
        // Get Earth position for transit line
        let earth_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
            .unwrap_or_default();

        // Get target asteroid position
        let asteroid_pos = if let Ok(body_state) = asteroids.get(deflector.target) {
            body_state.pos
        } else {
            continue;
        };

        // Get method-specific color
        let color = method_color(&deflector.payload);

        match &deflector.state {
            ContinuousDeflectorState::EnRoute { .. } => {
                // Draw dashed transit line from Earth to asteroid
                draw_dashed_line(
                    &mut gizmos,
                    earth_pos,
                    asteroid_pos,
                    color.with_alpha(0.5),
                    10,
                );
            }
            ContinuousDeflectorState::Operating { .. } => {
                // Draw spacecraft icon near asteroid
                let spacecraft_pos = asteroid_pos;
                draw_spacecraft_icon(
                    &mut gizmos,
                    spacecraft_pos,
                    color,
                );

                // Draw thrust direction indicator
                if let Ok(body_state) = asteroids.get(deflector.target) {
                    draw_thrust_arrow(
                        &mut gizmos,
                        spacecraft_pos,
                        body_state.vel,
                        &deflector.payload,
                        color,
                    );
                }
            }
            _ => {
                // Finished deflectors - just show completed icon
                draw_completed_icon(&mut gizmos, asteroid_pos, color);
            }
        }
    }
}

/// Get color for a deflection method.
fn method_color(payload: &ContinuousPayload) -> Color {
    match payload {
        ContinuousPayload::IonBeam { .. } => Color::srgba(0.0, 0.8, 1.0, 1.0),      // Cyan
        ContinuousPayload::GravityTractor { .. } => Color::srgba(0.8, 0.4, 1.0, 1.0), // Purple
        ContinuousPayload::LaserAblation { .. } => Color::srgba(1.0, 0.6, 0.2, 1.0),  // Orange
    }
}

/// Draw a dashed line between two points.
fn draw_dashed_line(
    gizmos: &mut Gizmos,
    start: bevy::math::DVec2,
    end: bevy::math::DVec2,
    color: Color,
    num_dashes: usize,
) {
    let start_render = Vec3::new(
        (start.x * RENDER_SCALE) as f32,
        (start.y * RENDER_SCALE) as f32,
        z_layers::TRAJECTORY,
    );
    let end_render = Vec3::new(
        (end.x * RENDER_SCALE) as f32,
        (end.y * RENDER_SCALE) as f32,
        z_layers::TRAJECTORY,
    );

    let delta = (end_render - start_render) / (num_dashes as f32 * 2.0);

    for i in 0..num_dashes {
        let dash_start = start_render + delta * (i as f32 * 2.0);
        let dash_end = start_render + delta * (i as f32 * 2.0 + 1.0);
        gizmos.line(dash_start, dash_end, color);
    }
}

/// Draw a simple spacecraft icon (diamond shape).
fn draw_spacecraft_icon(
    gizmos: &mut Gizmos,
    pos: bevy::math::DVec2,
    color: Color,
) {
    let center = Vec3::new(
        (pos.x * RENDER_SCALE) as f32,
        (pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Diamond shape size (in render units)
    let size = 0.02;

    let top = center + Vec3::new(0.0, size, 0.0);
    let right = center + Vec3::new(size, 0.0, 0.0);
    let bottom = center + Vec3::new(0.0, -size, 0.0);
    let left = center + Vec3::new(-size, 0.0, 0.0);

    gizmos.line(top, right, color);
    gizmos.line(right, bottom, color);
    gizmos.line(bottom, left, color);
    gizmos.line(left, top, color);
}

/// Draw thrust direction arrow.
fn draw_thrust_arrow(
    gizmos: &mut Gizmos,
    pos: bevy::math::DVec2,
    vel: bevy::math::DVec2,
    payload: &ContinuousPayload,
    color: Color,
) {
    use crate::continuous::thrust::compute_thrust_direction;

    let direction = payload.direction();
    let thrust_dir = compute_thrust_direction(vel, pos, direction);

    if thrust_dir.length() < 0.01 {
        return;
    }

    let center = Vec3::new(
        (pos.x * RENDER_SCALE) as f32,
        (pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Arrow pointing in thrust direction
    let arrow_length = 0.05;
    let arrow_end = center + Vec3::new(
        (thrust_dir.x * arrow_length) as f32,
        (thrust_dir.y * arrow_length) as f32,
        0.0,
    );

    gizmos.line(center, arrow_end, color);

    // Arrow head
    let head_size = 0.01;
    let perp = Vec3::new(-thrust_dir.y as f32, thrust_dir.x as f32, 0.0).normalize() * head_size;
    let back = (center - arrow_end).normalize() * head_size * 2.0;

    gizmos.line(arrow_end, arrow_end + back + perp, color);
    gizmos.line(arrow_end, arrow_end + back - perp, color);
}

/// Draw a completed mission icon (small circle).
fn draw_completed_icon(
    gizmos: &mut Gizmos,
    pos: bevy::math::DVec2,
    color: Color,
) {
    let center = Vec3::new(
        (pos.x * RENDER_SCALE) as f32,
        (pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Draw a small circle
    let radius = 0.01;
    let segments = 8;
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p1 = center + Vec3::new(angle1.cos() * radius, angle1.sin() * radius, 0.0);
        let p2 = center + Vec3::new(angle2.cos() * radius, angle2.sin() * radius, 0.0);

        gizmos.line(p1, p2, color.with_alpha(0.5));
    }
}
