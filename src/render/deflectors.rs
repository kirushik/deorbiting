//! Visualization for continuous deflection spacecraft.
//!
//! Renders:
//! - Transit trajectory (dashed line from Earth to asteroid)
//! - Operating spacecraft icon near asteroid
//! - Thrust direction indicator

use bevy::prelude::*;

use crate::asteroid::Asteroid;
use crate::camera::RENDER_SCALE;
use crate::continuous::{ContinuousDeflector, ContinuousDeflectorState, ContinuousPayload};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::types::{BodyState, SimulationTime};

use super::z_layers;

/// System to draw continuous deflector visualization.
pub fn draw_deflector_trajectories(
    deflectors: Query<(Entity, &ContinuousDeflector)>,
    asteroids: Query<&BodyState, With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    mut gizmos: Gizmos,
) {
    // Animation time for flickering effects
    let anim_time = sim_time.current as f32;

    // Count deflectors per target for offset calculation
    let mut target_counts: std::collections::HashMap<Entity, usize> =
        std::collections::HashMap::new();

    for (entity, deflector) in deflectors.iter() {
        // Get deflector index for this target (for visual offset)
        let deflector_index = *target_counts.entry(deflector.target).or_insert(0);
        target_counts.insert(deflector.target, deflector_index + 1);

        // Use entity bits for stable color variation
        let entity_hash = entity.to_bits() as u32;
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

        // Get method-specific color with hue variation for multiple deflectors
        let base_color = method_color(&deflector.payload);
        // Use entity_hash to create consistent but varied hue shift (0-0.15 range)
        let hue_shift = (entity_hash % 100) as f32 / 100.0 * 0.15;
        let color = shift_color_hue(base_color, hue_shift);

        // Calculate position offset for operating deflectors
        let pos_offset = deflector_offset(deflector_index);

        match &deflector.state {
            ContinuousDeflectorState::EnRoute { arrival_time } => {
                // Calculate interpolated spacecraft position
                let total_time = *arrival_time - deflector.launch_time;
                let elapsed = sim_time.current - deflector.launch_time;
                let progress = if total_time > 0.0 {
                    (elapsed / total_time).clamp(0.0, 1.0)
                } else {
                    1.0
                };

                // Apply position offset for visual differentiation of multiple en-route deflectors
                let target_pos = asteroid_pos + pos_offset;

                // Simple linear interpolation for spacecraft position (to offset target)
                let current_pos = earth_pos.lerp(target_pos, progress);

                // Draw traveled portion (solid line)
                draw_solid_line(&mut gizmos, earth_pos, current_pos, color);

                // Draw remaining portion (dashed line, semi-transparent)
                draw_dashed_line(
                    &mut gizmos,
                    current_pos,
                    target_pos,
                    color.with_alpha(0.4),
                    ((1.0 - progress) * 10.0).max(1.0) as usize,
                );

                // Draw spacecraft icon at current position
                draw_spacecraft_icon(&mut gizmos, current_pos, color);
            }
            ContinuousDeflectorState::Operating { .. } => {
                // Get asteroid velocity for thrust direction
                let asteroid_vel = asteroids
                    .get(deflector.target)
                    .map(|b| b.vel)
                    .unwrap_or_default();

                // Apply position offset for visual differentiation of multiple deflectors
                let offset_pos = asteroid_pos + pos_offset;

                // Draw method-specific visualization
                match &deflector.payload {
                    ContinuousPayload::IonBeam { .. } => {
                        draw_ion_beam_exhaust(
                            &mut gizmos,
                            offset_pos,
                            asteroid_vel,
                            &deflector.payload,
                            anim_time,
                            color,
                        );
                    }
                    ContinuousPayload::LaserAblation { .. } => {
                        draw_laser_beam(
                            &mut gizmos,
                            offset_pos,
                            asteroid_vel,
                            &deflector.payload,
                            anim_time,
                            color,
                        );
                    }
                    ContinuousPayload::SolarSail { sail_area_m2, .. } => {
                        draw_solar_sail(&mut gizmos, offset_pos, *sail_area_m2, color);
                    }
                    ContinuousPayload::GravityTractor {
                        hover_distance_m, ..
                    } => {
                        draw_gravity_tractor_field(
                            &mut gizmos,
                            offset_pos,
                            *hover_distance_m,
                            color,
                        );
                    }
                }

                // Draw spacecraft icon at offset position
                draw_spacecraft_icon(&mut gizmos, offset_pos, color);
            }
            _ => {
                // Finished deflectors - just show completed icon (with offset for multiple)
                let offset_pos = asteroid_pos + pos_offset;
                draw_completed_icon(&mut gizmos, offset_pos, color);
            }
        }
    }
}

/// Get color for a deflection method.
fn method_color(payload: &ContinuousPayload) -> Color {
    match payload {
        ContinuousPayload::IonBeam { .. } => Color::srgba(0.0, 0.8, 1.0, 1.0), // Cyan
        ContinuousPayload::GravityTractor { .. } => Color::srgba(0.8, 0.4, 1.0, 1.0), // Purple
        ContinuousPayload::LaserAblation { .. } => Color::srgba(1.0, 0.6, 0.2, 1.0), // Orange
        ContinuousPayload::SolarSail { .. } => Color::srgba(1.0, 0.9, 0.3, 1.0), // Yellow/Gold
    }
}

/// Shift a color's hue by a given amount (0.0 to 1.0 wraps around).
/// This allows multiple deflectors to have visually distinct colors.
fn shift_color_hue(color: Color, hue_shift: f32) -> Color {
    // Convert sRGBA to HSL-like representation
    let rgba = color.to_srgba();
    let r = rgba.red;
    let g = rgba.green;
    let b = rgba.blue;
    let a = rgba.alpha;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    // Calculate hue (0.0 to 1.0)
    let hue = if delta < 0.001 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0) / 6.0
    } else if max == g {
        ((b - r) / delta + 2.0) / 6.0
    } else {
        ((r - g) / delta + 4.0) / 6.0
    };

    // Calculate saturation and lightness
    let lightness = (max + min) / 2.0;
    let saturation = if delta < 0.001 {
        0.0
    } else {
        delta / (1.0 - (2.0 * lightness - 1.0).abs())
    };

    // Apply hue shift (wrapping around)
    let new_hue = (hue + hue_shift).rem_euclid(1.0);

    // Convert back to RGB
    let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = c * (1.0 - ((new_hue * 6.0).rem_euclid(2.0) - 1.0).abs());
    let m = lightness - c / 2.0;

    let (r, g, b) = match (new_hue * 6.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Color::srgba(r + m, g + m, b + m, a)
}

/// Calculate position offset for multiple deflectors at same target.
/// Returns a small offset vector to spread them out visually.
fn deflector_offset(deflector_index: usize) -> bevy::math::DVec2 {
    // Arrange in a small circle around the target
    // Each deflector gets a 45-degree offset
    let angle = (deflector_index as f64) * std::f64::consts::PI / 4.0;
    let offset_distance = 0.03 / RENDER_SCALE; // Small offset in simulation units
    bevy::math::DVec2::new(angle.cos() * offset_distance, angle.sin() * offset_distance)
}

/// Draw a solid line between two points.
fn draw_solid_line(
    gizmos: &mut Gizmos,
    start: bevy::math::DVec2,
    end: bevy::math::DVec2,
    color: Color,
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

    gizmos.line(start_render, end_render, color);
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
fn draw_spacecraft_icon(gizmos: &mut Gizmos, pos: bevy::math::DVec2, color: Color) {
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

/// Draw a completed mission icon (small circle).
fn draw_completed_icon(gizmos: &mut Gizmos, pos: bevy::math::DVec2, color: Color) {
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

// =============================================================================
// Method-specific operating visualizations
// =============================================================================

/// Draw ion beam exhaust - cone of flickering lines pointing in thrust direction.
fn draw_ion_beam_exhaust(
    gizmos: &mut Gizmos,
    asteroid_pos: bevy::math::DVec2,
    asteroid_vel: bevy::math::DVec2,
    payload: &ContinuousPayload,
    anim_time: f32,
    color: Color,
) {
    use crate::continuous::thrust::compute_thrust_direction;

    let direction = payload.direction();
    let thrust_dir = compute_thrust_direction(asteroid_vel, asteroid_pos, direction);

    if thrust_dir.length() < 0.01 {
        return;
    }

    let center = Vec3::new(
        (asteroid_pos.x * RENDER_SCALE) as f32,
        (asteroid_pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Ion beam exhaust is opposite to thrust direction (Newton's 3rd law)
    let exhaust_dir = -thrust_dir.normalize();
    let exhaust_dir_3d = Vec3::new(exhaust_dir.x as f32, exhaust_dir.y as f32, 0.0);

    // Perpendicular direction for cone spread
    let perp_dir = Vec3::new(-exhaust_dir.y as f32, exhaust_dir.x as f32, 0.0);

    // Draw 5 exhaust lines in a cone pattern with flickering
    let cone_half_angle = 0.3; // radians
    let exhaust_length = 0.08;

    for i in 0..5 {
        // Spread across the cone
        let spread = (i as f32 - 2.0) / 2.0 * cone_half_angle;

        // Flickering intensity
        let flicker = 0.7 + 0.3 * ((anim_time * 10.0 + i as f32 * 1.7).sin());
        let line_alpha = flicker * 0.8;

        // Line direction with spread
        let line_dir = exhaust_dir_3d * spread.cos() + perp_dir * spread.sin();
        let line_end = center + line_dir.normalize() * exhaust_length * flicker;

        // Gradient: bright cyan at start, fading to transparent
        let Srgba {
            red, green, blue, ..
        } = color.to_srgba();
        let start_color = Color::srgba(red, green, blue, line_alpha);
        let end_color = Color::srgba(red * 0.5, green * 0.5, blue, line_alpha * 0.3);

        // Draw line with approximate gradient (two segments)
        let mid = center + line_dir.normalize() * exhaust_length * 0.5 * flicker;
        gizmos.line(center, mid, start_color);
        gizmos.line(mid, line_end, end_color);
    }

    // Draw small particle dots at exhaust ends
    for i in 0..3 {
        let spread = (i as f32 - 1.0) / 1.0 * cone_half_angle * 0.5;
        let particle_dist =
            exhaust_length * (0.6 + 0.4 * ((anim_time * 8.0 + i as f32 * 2.3).sin()));
        let line_dir = exhaust_dir_3d * spread.cos() + perp_dir * spread.sin();
        let particle_pos = center + line_dir.normalize() * particle_dist;

        let dot_size = 0.003;
        let particle_alpha = 0.6 + 0.4 * ((anim_time * 12.0 + i as f32).sin());
        let particle_color = color.with_alpha(particle_alpha);

        gizmos.line(
            particle_pos - Vec3::X * dot_size,
            particle_pos + Vec3::X * dot_size,
            particle_color,
        );
    }
}

/// Draw laser ablation beam - solid line from spacecraft to asteroid with glow at impact.
fn draw_laser_beam(
    gizmos: &mut Gizmos,
    asteroid_pos: bevy::math::DVec2,
    asteroid_vel: bevy::math::DVec2,
    payload: &ContinuousPayload,
    anim_time: f32,
    color: Color,
) {
    use crate::continuous::thrust::compute_thrust_direction;

    let direction = payload.direction();
    let thrust_dir = compute_thrust_direction(asteroid_vel, asteroid_pos, direction);

    if thrust_dir.length() < 0.01 {
        return;
    }

    let center = Vec3::new(
        (asteroid_pos.x * RENDER_SCALE) as f32,
        (asteroid_pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Spacecraft position is offset from asteroid in thrust direction
    let spacecraft_offset = 0.05;
    let spacecraft_pos = center
        + Vec3::new(
            thrust_dir.x as f32 * spacecraft_offset,
            thrust_dir.y as f32 * spacecraft_offset,
            0.0,
        );

    // Laser beam from spacecraft to asteroid
    // Flickering intensity
    let flicker = 0.8 + 0.2 * (anim_time * 20.0).sin();
    let beam_color = Color::srgba(1.0, 0.4, 0.1, flicker);

    gizmos.line(spacecraft_pos, center, beam_color);

    // Glow circle at impact point (pulsing)
    let glow_radius = 0.012 + 0.004 * (anim_time * 15.0).sin();
    let glow_alpha = 0.6 + 0.3 * (anim_time * 15.0).cos();
    let glow_color = Color::srgba(1.0, 0.6, 0.2, glow_alpha);

    let segments = 8;
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p1 = center + Vec3::new(angle1.cos() * glow_radius, angle1.sin() * glow_radius, 0.0);
        let p2 = center + Vec3::new(angle2.cos() * glow_radius, angle2.sin() * glow_radius, 0.0);

        gizmos.line(p1, p2, glow_color);
    }

    // Inner bright spot
    let inner_radius = glow_radius * 0.4;
    let inner_color = Color::srgba(1.0, 0.9, 0.6, glow_alpha * 0.8);
    for i in 0..4 {
        let angle1 = (i as f32 / 4.0) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / 4.0) * std::f32::consts::TAU;

        let p1 = center
            + Vec3::new(
                angle1.cos() * inner_radius,
                angle1.sin() * inner_radius,
                0.0,
            );
        let p2 = center
            + Vec3::new(
                angle2.cos() * inner_radius,
                angle2.sin() * inner_radius,
                0.0,
            );

        gizmos.line(p1, p2, inner_color);
    }

    // Draw spacecraft position marker (small triangle)
    let sc_size = 0.015;
    let sc_dir = Vec3::new(-thrust_dir.x as f32, -thrust_dir.y as f32, 0.0).normalize();
    let sc_perp = Vec3::new(-sc_dir.y, sc_dir.x, 0.0);

    let sc_tip = spacecraft_pos + sc_dir * sc_size;
    let sc_left = spacecraft_pos - sc_dir * sc_size * 0.5 + sc_perp * sc_size * 0.5;
    let sc_right = spacecraft_pos - sc_dir * sc_size * 0.5 - sc_perp * sc_size * 0.5;

    gizmos.line(sc_tip, sc_left, color);
    gizmos.line(sc_left, sc_right, color);
    gizmos.line(sc_right, sc_tip, color);
}

/// Draw solar sail - large reflective square/diamond perpendicular to sun direction.
fn draw_solar_sail(
    gizmos: &mut Gizmos,
    asteroid_pos: bevy::math::DVec2,
    sail_area_m2: f64,
    color: Color,
) {
    let center = Vec3::new(
        (asteroid_pos.x * RENDER_SCALE) as f32,
        (asteroid_pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Direction to sun (from asteroid)
    let to_sun = -asteroid_pos.normalize();
    let sun_dir = Vec3::new(to_sun.x as f32, to_sun.y as f32, 0.0);

    // Sail is perpendicular to sun direction
    let sail_perp = Vec3::new(-sun_dir.y, sun_dir.x, 0.0);

    // Sail size based on area (square root, then scaled for visibility)
    // sqrt(1,000,000 m²) = 1000m, scale down significantly for render
    let sail_side = (sail_area_m2.sqrt() / 1e7) as f32 * 0.1;
    let sail_size = sail_side.clamp(0.03, 0.15);

    // Offset sail slightly toward sun (it's between asteroid and sun)
    let sail_center = center + sun_dir * 0.03;

    // Draw diamond shape
    let top = sail_center + sail_perp * sail_size;
    let bottom = sail_center - sail_perp * sail_size;
    let front = sail_center + sun_dir * sail_size * 0.3;
    let back = sail_center - sun_dir * sail_size * 0.3;

    // Bright reflective color (yellow-white)
    let sail_color = Color::srgba(1.0, 0.95, 0.7, 0.9);

    gizmos.line(top, front, sail_color);
    gizmos.line(front, bottom, sail_color);
    gizmos.line(bottom, back, sail_color);
    gizmos.line(back, top, sail_color);

    // Draw support struts (cross pattern)
    let strut_color = color.with_alpha(0.6);
    gizmos.line(top, bottom, strut_color);
    gizmos.line(front, back, strut_color);

    // Draw connecting tether to asteroid
    let tether_color = color.with_alpha(0.4);
    gizmos.line(sail_center, center, tether_color);
}

/// Draw gravity tractor field lines - curved dashed lines between spacecraft and asteroid.
fn draw_gravity_tractor_field(
    gizmos: &mut Gizmos,
    asteroid_pos: bevy::math::DVec2,
    hovering_distance_m: f64,
    color: Color,
) {
    let center = Vec3::new(
        (asteroid_pos.x * RENDER_SCALE) as f32,
        (asteroid_pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Spacecraft hovers at a distance (scale for visibility)
    // 200m real distance → ~0.04 render units offset
    let hover_scale = (hovering_distance_m / 5000.0) as f32;
    let hover_dist = hover_scale.clamp(0.03, 0.08);

    // Draw 3 curved field lines at different angles
    let field_color = color.with_alpha(0.5);

    for i in 0..3 {
        let angle_offset = (i as f32 - 1.0) * 0.4; // -0.4, 0, +0.4 radians

        // Spacecraft position for this field line
        let sc_angle = angle_offset;
        let sc_offset = Vec3::new(
            sc_angle.cos() * hover_dist,
            sc_angle.sin() * hover_dist,
            0.0,
        );
        let sc_pos = center + sc_offset;

        // Draw curved field line using multiple segments
        draw_curved_field_line(gizmos, center, sc_pos, field_color, 6);

        // Small dot at spacecraft position
        let dot_size = 0.005;
        gizmos.line(
            sc_pos - Vec3::X * dot_size,
            sc_pos + Vec3::X * dot_size,
            color,
        );
        gizmos.line(
            sc_pos - Vec3::Y * dot_size,
            sc_pos + Vec3::Y * dot_size,
            color,
        );
    }
}

/// Draw a curved dashed field line between two points.
fn draw_curved_field_line(
    gizmos: &mut Gizmos,
    start: Vec3,
    end: Vec3,
    color: Color,
    segments: usize,
) {
    let mid = (start + end) / 2.0;
    let diff = end - start;
    let perp = Vec3::new(-diff.y, diff.x, 0.0).normalize();

    // Curve outward
    let curve_amount = diff.length() * 0.3;
    let control = mid + perp * curve_amount;

    // Draw dashed quadratic bezier approximation
    let mut prev = start;
    for i in 1..=segments {
        let t = i as f32 / segments as f32;

        // Quadratic bezier: (1-t)²P0 + 2(1-t)tP1 + t²P2
        let one_minus_t = 1.0 - t;
        let point =
            start * one_minus_t * one_minus_t + control * 2.0 * one_minus_t * t + end * t * t;

        // Draw every other segment (dashed)
        if i % 2 == 1 {
            gizmos.line(prev, point, color);
        }
        prev = point;
    }
}
