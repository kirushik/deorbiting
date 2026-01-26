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
use crate::types::AU_TO_METERS;
use crate::types::{BodyState, SimulationTime};

use super::bodies::{CelestialBody, EffectiveVisualRadius};
use super::z_layers;

/// Base visual size unit (fraction of 1 AU in render space).
/// This matches the scale used by interceptor icons.
const VISUAL_UNIT: f32 = 0.01 * AU_TO_METERS as f32 * RENDER_SCALE as f32;

// ============================================================================
// Beam Occlusion Helpers
// ============================================================================

/// Calculate the entry point of a line into a circle (line-circle intersection).
///
/// Returns Some(t) where the line enters the circle at `line_start + t * line_dir`,
/// or None if no intersection occurs within the valid range.
///
/// # Parameters
/// - `line_start`: Start point of the line
/// - `line_dir`: Direction of the line (should be normalized)
/// - `line_length`: Maximum distance along the line to check
/// - `circle_center`: Center of the circle
/// - `circle_radius`: Radius of the circle
fn line_circle_entry(
    line_start: Vec3,
    line_dir: Vec3,
    line_length: f32,
    circle_center: Vec3,
    circle_radius: f32,
) -> Option<f32> {
    // Project in 2D (ignore Z)
    let to_center = circle_center - line_start;
    let to_center_2d = Vec3::new(to_center.x, to_center.y, 0.0);
    let line_dir_2d = Vec3::new(line_dir.x, line_dir.y, 0.0);

    // Project circle center onto line
    let t_closest = to_center_2d.dot(line_dir_2d);

    // Skip if circle is behind start or past end
    if t_closest < -circle_radius || t_closest > line_length + circle_radius {
        return None;
    }

    // Distance from circle center to line
    let closest_point = line_start + line_dir * t_closest;
    let dist_to_line = (circle_center - closest_point).length();

    // No intersection if line passes outside circle
    if dist_to_line >= circle_radius {
        return None;
    }

    // Calculate entry point using quadratic formula
    let discriminant = circle_radius * circle_radius - dist_to_line * dist_to_line;
    if discriminant <= 0.0 {
        return None;
    }

    let half_chord = discriminant.sqrt();
    let t_entry = t_closest - half_chord;

    // Only return if entry is within valid segment and positive
    if t_entry > 0.0 && t_entry < line_length {
        Some(t_entry)
    } else {
        None
    }
}

/// Check if a beam is occluded by any celestial body and find the effective endpoint.
///
/// Returns (effective_endpoint, is_occluded) where:
/// - effective_endpoint is either the original target or the first occlusion point
/// - is_occluded is true if the beam is blocked by an occluder
///
/// # Parameters
/// - `beam_start`: Starting point of the beam
/// - `beam_end`: Target endpoint of the beam
/// - `occluders`: List of (position, radius) pairs for potential occluders
/// - `skip_near_start_dist`: Occluders within this distance of start are ignored
///   (e.g., Earth shouldn't block its own beam)
fn check_beam_occlusion(
    beam_start: Vec3,
    beam_end: Vec3,
    occluders: &[(Vec3, f32)],
    skip_near_start_dist_mult: f32,
) -> (Vec3, bool) {
    let beam_vec = beam_end - beam_start;
    let beam_dist = beam_vec.length();
    if beam_dist < 0.01 {
        return (beam_end, false);
    }
    let beam_dir = beam_vec / beam_dist;

    let mut effective_end = beam_end;
    let mut is_occluded = false;

    for &(body_pos, body_radius) in occluders {
        // Skip bodies very close to beam start (e.g., Earth for Earth-based laser)
        let start_to_body = (body_pos - beam_start).length();
        if start_to_body < body_radius * skip_near_start_dist_mult {
            continue;
        }

        // Check for intersection
        if let Some(t_entry) = line_circle_entry(beam_start, beam_dir, beam_dist, body_pos, body_radius) {
            let occlusion_point = beam_start + beam_dir * t_entry;
            let current_dist = (effective_end - beam_start).length();
            if t_entry < current_dist {
                effective_end = occlusion_point;
                is_occluded = true;
            }
        }
    }

    (effective_end, is_occluded)
}

/// System to draw continuous deflector visualization.
pub fn draw_deflector_trajectories(
    deflectors: Query<(Entity, &ContinuousDeflector)>,
    asteroids: Query<&BodyState, With<Asteroid>>,
    celestial_bodies: Query<(&Transform, &CelestialBody, &EffectiveVisualRadius)>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    mut gizmos: Gizmos,
) {
    // Animation time for flickering effects
    let anim_time = sim_time.current as f32;

    // Collect occluder data (position + visual radius) for Sun and planets
    // Used by laser beam to check for visual occlusion
    let occluders: Vec<(Vec3, f32)> = celestial_bodies
        .iter()
        .filter(|(_, body, _)| {
            // Only Sun and planets can occlude (not moons - they're too small visually)
            body.id == CelestialBodyId::Sun || body.id.parent().is_none()
        })
        .map(|(transform, _, radius)| (transform.translation, radius.0))
        .collect();

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
                // Calculate progress along transfer arc
                let total_time = *arrival_time - deflector.launch_time;
                let elapsed = sim_time.current - deflector.launch_time;
                let progress = if total_time > 0.0 {
                    (elapsed / total_time).clamp(0.0, 1.0)
                } else {
                    1.0
                };

                // Apply position offset for visual differentiation of multiple en-route deflectors
                let target_pos = asteroid_pos + pos_offset;

                // Use transfer arc for curved trajectory visualization
                if !deflector.transfer_arc.is_empty() {
                    let arc_len = deflector.transfer_arc.len();
                    let progress_idx = (progress * arc_len as f64) as usize;
                    let progress_idx = progress_idx.min(arc_len.saturating_sub(1));

                    // Draw traveled portion of arc (solid)
                    for i in 0..progress_idx {
                        let p0 = deflector.transfer_arc[i];
                        let p1 = deflector.transfer_arc[i + 1];
                        draw_solid_line(&mut gizmos, p0, p1, color);
                    }

                    // Draw remaining portion of arc (dashed, semi-transparent)
                    for i in progress_idx..arc_len.saturating_sub(1) {
                        let p0 = deflector.transfer_arc[i];
                        let p1 = deflector.transfer_arc[i + 1];
                        // Use single dash segment for each arc segment
                        draw_dashed_line(&mut gizmos, p0, p1, color.with_alpha(0.4), 2);
                    }

                    // Draw spacecraft icon at current position along arc
                    let current_pos = deflector.transfer_arc[progress_idx];
                    draw_spacecraft_icon(&mut gizmos, current_pos, color);
                } else {
                    // Fallback to linear interpolation if no arc available (e.g., instant laser)
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
                            earth_pos,
                            &occluders,
                            anim_time,
                            color,
                        );
                    }
                    ContinuousPayload::SolarSail { sail_area_m2, .. } => {
                        draw_solar_sail(&mut gizmos, offset_pos, *sail_area_m2, color);
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

    // Diamond shape size using VISUAL_UNIT for proper scaling
    let size = VISUAL_UNIT;

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

    // Draw a small circle using VISUAL_UNIT for proper scaling
    let radius = VISUAL_UNIT * 0.5;
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

    // Use VISUAL_UNIT for proper scaling
    let exhaust_length = VISUAL_UNIT * 3.0;

    // Draw spacecraft body first (small box)
    let sc_offset = VISUAL_UNIT * 1.0;
    let sc_pos = center - exhaust_dir_3d * sc_offset;
    let sc_size = VISUAL_UNIT * 0.4;
    let sc_perp = perp_dir * sc_size;
    let sc_color = color.with_alpha(0.8);

    gizmos.line(sc_pos + sc_perp, sc_pos - sc_perp, sc_color);
    gizmos.line(
        sc_pos + sc_perp,
        sc_pos + exhaust_dir_3d * sc_size + sc_perp * 0.5,
        sc_color,
    );
    gizmos.line(
        sc_pos - sc_perp,
        sc_pos + exhaust_dir_3d * sc_size - sc_perp * 0.5,
        sc_color,
    );

    // Draw 7 exhaust lines in a cone pattern with flickering
    let cone_half_angle = 0.35; // radians

    for i in 0..7 {
        // Spread across the cone
        let spread = (i as f32 - 3.0) / 3.0 * cone_half_angle;

        // Flickering intensity
        let flicker = 0.7 + 0.3 * ((anim_time * 10.0 + i as f32 * 1.7).sin());
        let line_alpha = flicker * 0.9;

        // Line direction with spread
        let line_dir = exhaust_dir_3d * spread.cos() + perp_dir * spread.sin();
        let line_start = sc_pos + exhaust_dir_3d * sc_size * 0.5;
        let line_end = line_start + line_dir.normalize() * exhaust_length * flicker;

        // Gradient: bright cyan at start, fading to transparent
        let Srgba {
            red, green, blue, ..
        } = color.to_srgba();
        let start_color = Color::srgba(red, green, blue, line_alpha);
        let end_color = Color::srgba(red * 0.5, green * 0.5, blue, line_alpha * 0.2);

        // Draw line with approximate gradient (three segments)
        let seg1 = line_start + line_dir.normalize() * exhaust_length * 0.33 * flicker;
        let seg2 = line_start + line_dir.normalize() * exhaust_length * 0.66 * flicker;
        gizmos.line(line_start, seg1, start_color);
        gizmos.line(
            seg1,
            seg2,
            Color::srgba(red * 0.7, green * 0.7, blue, line_alpha * 0.6),
        );
        gizmos.line(seg2, line_end, end_color);
    }

    // Draw particle dots at exhaust ends
    let dot_size = VISUAL_UNIT * 0.15;
    for i in 0..5 {
        let spread = (i as f32 - 2.0) / 2.0 * cone_half_angle * 0.6;
        let particle_dist =
            exhaust_length * (0.6 + 0.4 * ((anim_time * 8.0 + i as f32 * 2.3).sin()));
        let line_dir = exhaust_dir_3d * spread.cos() + perp_dir * spread.sin();
        let particle_pos = sc_pos + line_dir.normalize() * particle_dist;

        let particle_alpha = 0.5 + 0.3 * ((anim_time * 12.0 + i as f32).sin());
        let particle_color = color.with_alpha(particle_alpha);

        gizmos.line(
            particle_pos - Vec3::X * dot_size,
            particle_pos + Vec3::X * dot_size,
            particle_color,
        );
        gizmos.line(
            particle_pos - Vec3::Y * dot_size,
            particle_pos + Vec3::Y * dot_size,
            particle_color,
        );
    }
}

/// Draw laser ablation beam - full beam from Earth toward asteroid with effects and occlusion.
///
/// The beam is drawn from Earth's position to the asteroid, with:
/// - Traveling energy pulses along the beam
/// - Pulsing/flickering intensity
/// - Ablation glow and plume at the impact point
/// - Occlusion: beam stops if it passes through a celestial body's visual representation
fn draw_laser_beam(
    gizmos: &mut Gizmos,
    asteroid_pos: bevy::math::DVec2,
    earth_pos: bevy::math::DVec2,
    occluders: &[(Vec3, f32)],
    anim_time: f32,
    _color: Color,
) {
    let asteroid_render = Vec3::new(
        (asteroid_pos.x * RENDER_SCALE) as f32,
        (asteroid_pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    let earth_render = Vec3::new(
        (earth_pos.x * RENDER_SCALE) as f32,
        (earth_pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT + 0.1,
    );

    // Direction and distance from Earth to asteroid
    let beam_vec = asteroid_render - earth_render;
    let beam_dist = beam_vec.length();
    if beam_dist < 0.01 {
        return;
    }
    let beam_dir = beam_vec / beam_dist;

    // Check for occlusion by celestial bodies using shared helper
    // Skip bodies within 2x their radius of Earth (Earth shouldn't block its own beam)
    let (effective_end, is_occluded) = check_beam_occlusion(
        earth_render,
        asteroid_render,
        occluders,
        2.0, // skip_near_start_dist_mult
    );

    // Flickering intensity for energy effect
    let flicker = 0.7 + 0.3 * (anim_time * 30.0).sin().abs();

    // If occluded, draw a dashed line from Earth to occlusion point only
    if is_occluded {
        let occluded_dist = (effective_end - earth_render).length();
        let dash_color = Color::srgba(1.0, 0.3, 0.1, 0.4); // Dimmer, semi-transparent
        let num_dashes = (occluded_dist / (VISUAL_UNIT * 0.5)).max(5.0) as usize;
        let dash_delta = (effective_end - earth_render) / (num_dashes as f32 * 2.0);

        for i in 0..num_dashes {
            let dash_start = earth_render + dash_delta * (i as f32 * 2.0);
            let dash_end = earth_render + dash_delta * (i as f32 * 2.0 + 1.0);
            gizmos.line(dash_start, dash_end, dash_color);
        }

        // Draw "blocked" indicator at the occlusion point
        let block_color = Color::srgba(1.0, 0.2, 0.1, 0.6 * flicker);
        let block_size = VISUAL_UNIT * 0.5;
        // Draw X at occlusion point
        let p1 = effective_end + Vec3::new(-block_size, -block_size, 0.0);
        let p2 = effective_end + Vec3::new(block_size, block_size, 0.0);
        let p3 = effective_end + Vec3::new(-block_size, block_size, 0.0);
        let p4 = effective_end + Vec3::new(block_size, -block_size, 0.0);
        gizmos.line(p1, p2, block_color);
        gizmos.line(p3, p4, block_color);
        return; // Don't draw impact effects if occluded
    }

    // Active beam (not occluded): draw solid beam with effects
    let perp = Vec3::new(-beam_dir.y, beam_dir.x, 0.0);
    let beam_width = VISUAL_UNIT * 0.1;

    // Main beam core (multiple parallel lines for thickness)
    for i in -2..=2 {
        let offset = perp * (i as f32 * beam_width * 0.2);
        let alpha = flicker * (1.0 - (i as f32).abs() * 0.15);
        let line_color = Color::srgba(1.0, 0.3, 0.1, alpha * 0.8);
        gizmos.line(earth_render + offset, asteroid_render + offset, line_color);
    }

    // Traveling energy pulses along the beam
    let pulse_count = 8;
    let pulse_speed = 0.3; // Fraction of beam per second
    let pulse_size = VISUAL_UNIT * 0.3;

    for i in 0..pulse_count {
        // Each pulse travels from Earth to asteroid
        let base_offset = i as f32 / pulse_count as f32;
        let pulse_t = (base_offset + anim_time * pulse_speed).fract();
        let pulse_pos_along_beam = pulse_t * beam_dist;

        let pulse_center = earth_render + beam_dir * pulse_pos_along_beam;

        // Pulse brightness varies (brighter in the middle of its cycle)
        let pulse_brightness = 0.5 + 0.5 * (pulse_t * std::f32::consts::TAU).sin().abs();
        let pulse_alpha = pulse_brightness * flicker;

        // Draw pulse as a bright spot (small circle)
        let pulse_color = Color::srgba(1.0, 0.6, 0.3, pulse_alpha);
        let segments = 6;
        for j in 0..segments {
            let angle1 = (j as f32 / segments as f32) * std::f32::consts::TAU;
            let angle2 = ((j + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let p1 = pulse_center
                + Vec3::new(angle1.cos() * pulse_size, angle1.sin() * pulse_size, 0.0);
            let p2 = pulse_center
                + Vec3::new(angle2.cos() * pulse_size, angle2.sin() * pulse_size, 0.0);
            gizmos.line(p1, p2, pulse_color);
        }
    }

    // Impact effects at asteroid (only if not occluded)
    let glow_radius = VISUAL_UNIT * (0.4 + 0.15 * (anim_time * 15.0).sin());
    let glow_alpha = 0.6 + 0.3 * (anim_time * 15.0).cos();
    let glow_color = Color::srgba(1.0, 0.6, 0.2, glow_alpha);

    // Outer glow circle
    let segments = 12;
    for i in 0..segments {
        let angle1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
        let p1 = asteroid_render
            + Vec3::new(angle1.cos() * glow_radius, angle1.sin() * glow_radius, 0.0);
        let p2 = asteroid_render
            + Vec3::new(angle2.cos() * glow_radius, angle2.sin() * glow_radius, 0.0);
        gizmos.line(p1, p2, glow_color);
    }

    // Inner bright spot (plasma/ablation point)
    let inner_radius = glow_radius * 0.5;
    let inner_color = Color::srgba(1.0, 0.95, 0.8, glow_alpha);
    for i in 0..6 {
        let angle1 = (i as f32 / 6.0) * std::f32::consts::TAU;
        let angle2 = ((i + 1) as f32 / 6.0) * std::f32::consts::TAU;
        let p1 = asteroid_render
            + Vec3::new(
                angle1.cos() * inner_radius,
                angle1.sin() * inner_radius,
                0.0,
            );
        let p2 = asteroid_render
            + Vec3::new(
                angle2.cos() * inner_radius,
                angle2.sin() * inner_radius,
                0.0,
            );
        gizmos.line(p1, p2, inner_color);
    }

    // Ablation plume - particles flying away from impact (toward Earth)
    let plume_dir = -beam_dir;
    let plume_perp = Vec3::new(plume_dir.y, -plume_dir.x, 0.0);
    let plume_color = Color::srgba(0.9, 0.7, 0.4, 0.5 * flicker);

    for i in 0..5 {
        let spread = (i as f32 - 2.0) * 0.3;
        let length = VISUAL_UNIT * (0.8 + (anim_time * 10.0 + i as f32).sin().abs() * 0.4);
        let start = asteroid_render + plume_perp * spread * VISUAL_UNIT * 0.3;
        let end = start + plume_dir * length;
        gizmos.line(start, end, plume_color);
    }
}

/// Draw solar sail - large reflective square/diamond perpendicular to sun direction.
/// Includes visualization of incoming solar rays and reflected beams.
fn draw_solar_sail(
    gizmos: &mut Gizmos,
    asteroid_pos: bevy::math::DVec2,
    sail_area_m2: f64,
    _color: Color,
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

    // Sail size: base size with scaling by area, using VISUAL_UNIT for proper visibility
    // 1 km² (1e6 m²) = 1x base size, 10 km² (1e7 m²) = ~3x base size
    let area_factor = (sail_area_m2 / 1e6).sqrt().clamp(0.5, 4.0) as f32;
    let sail_size = VISUAL_UNIT * 1.5 * area_factor;

    // Offset sail toward sun (it's between asteroid and sun)
    let sail_offset = VISUAL_UNIT * 2.0;
    let sail_center = center + sun_dir * sail_offset;

    // Draw diamond shape
    let top = sail_center + sail_perp * sail_size;
    let bottom = sail_center - sail_perp * sail_size;
    let front = sail_center + sun_dir * sail_size * 0.3;
    let back = sail_center - sun_dir * sail_size * 0.3;

    // Bright reflective color (golden-white)
    let sail_color = Color::srgba(1.0, 0.95, 0.7, 0.9);

    gizmos.line(top, front, sail_color);
    gizmos.line(front, bottom, sail_color);
    gizmos.line(bottom, back, sail_color);
    gizmos.line(back, top, sail_color);

    // Draw support struts (cross pattern)
    let strut_color = Color::srgba(0.9, 0.85, 0.6, 0.6);
    gizmos.line(top, bottom, strut_color);
    gizmos.line(front, back, strut_color);

    // Draw connecting tether to asteroid
    let tether_color = Color::srgba(0.8, 0.8, 0.6, 0.4);
    gizmos.line(sail_center, center, tether_color);

    // Draw incoming solar rays (dashed yellow lines from Sun direction)
    let ray_color = Color::srgba(1.0, 0.9, 0.2, 0.7);
    let reflected_color = Color::srgba(1.0, 0.95, 0.7, 0.9);
    let ray_count = 5;
    let ray_length = VISUAL_UNIT * 4.0;
    let reflected_length = VISUAL_UNIT * 2.5;

    for i in 0..ray_count {
        let offset = (i as f32 - (ray_count - 1) as f32 / 2.0) * sail_size * 0.4;
        let ray_hit_point = sail_center + sail_perp * offset;
        let ray_start = ray_hit_point - sun_dir * ray_length;

        // Draw dashed incoming ray (from sun toward sail)
        let num_dashes = 6;
        let dash_delta = (ray_hit_point - ray_start) / (num_dashes as f32 * 2.0);
        for j in 0..num_dashes {
            let dash_start = ray_start + dash_delta * (j as f32 * 2.0);
            let dash_end = ray_start + dash_delta * (j as f32 * 2.0 + 1.0);
            gizmos.line(dash_start, dash_end, ray_color);
        }

        // Draw solid reflected ray (bouncing away from sail, pushing asteroid)
        let reflected_end = ray_hit_point + sun_dir * reflected_length;
        gizmos.line(ray_hit_point, reflected_end, reflected_color);
    }
}
