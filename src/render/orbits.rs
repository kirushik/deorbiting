//! Orbit path rendering using Bevy Gizmos.
//!
//! Draws *idealized* Keplerian ellipses for planet orbit paths, aligned so the planet's
//! *current* ephemeris position lies on the rendered curve.
//!
//! Rationale:
//! - High-fidelity ephemerides (tables) are not perfectly two-body Keplerian ellipses.
//! - Sampling “one full turn” in time can under/overshoot and create artifacts (chords/overlaps).
//! - For clean visuals, we use the baked Kepler ellipse shape (a/e/ω) and rotate it so that it
//!   passes through the current ephemeris position.
//!
//! Notes:
//! - This is a rendering-only approximation (the physics/ephemeris remains the source of truth).
//! - The orbit path may rotate slightly over time as the “best fit” changes with the ephemeris.

use bevy::prelude::*;

use crate::camera::RENDER_SCALE;
use crate::ephemeris::{all_bodies, CelestialBodyId, Ephemeris};
use crate::render::z_layers;
use crate::types::SimulationTime;

use super::bodies::{CelestialBody, EffectiveVisualRadius};
use super::scaling::MARGIN_FRACTION;

/// Plugin providing orbit path visualization.
pub struct OrbitPathPlugin;

impl Plugin for OrbitPathPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitPathSettings>()
            .init_resource::<SoiSettings>()
            .init_resource::<DangerZoneSettings>();
        // Systems are added by RenderPlugin with proper ordering
    }
}


/// Settings for orbit path rendering.
#[derive(Resource)]
pub struct OrbitPathSettings {
    /// Whether to show orbit paths.
    pub visible: bool,
    /// Number of segments for drawing the ellipse (higher = smoother).
    pub segments: u32,
    /// Alpha value for orbit path color.
    pub alpha: f32,
    /// Exaggeration factor applied to the eccentricity when fitting (rendering only).
    ///
    /// This allows you to tune how “ellipse-like” the path appears without changing physics.
    pub eccentricity_scale: f64,
    /// Dash pattern: draw N segments, then skip M segments, repeating.
    ///
    /// Set to (1, 0) for a solid line.
    pub dash_on: u32,
    pub dash_off: u32,
}

impl Default for OrbitPathSettings {
    fn default() -> Self {
        Self {
            visible: true,
            segments: 256,
            alpha: 0.3,
            eccentricity_scale: 1.0,
            dash_on: 2,
            dash_off: 3,
        }
    }
}

/// Get a dim color for orbit path based on body ID.
fn orbit_color(id: CelestialBodyId, alpha: f32) -> Color {
    match id {
        CelestialBodyId::Mercury => Color::srgba(0.6, 0.6, 0.6, alpha),
        CelestialBodyId::Venus => Color::srgba(0.9, 0.85, 0.7, alpha),
        CelestialBodyId::Earth => Color::srgba(0.2, 0.5, 0.8, alpha),
        CelestialBodyId::Mars => Color::srgba(0.8, 0.4, 0.2, alpha),
        CelestialBodyId::Jupiter => Color::srgba(0.8, 0.7, 0.6, alpha),
        CelestialBodyId::Saturn => Color::srgba(0.9, 0.85, 0.6, alpha),
        CelestialBodyId::Uranus => Color::srgba(0.6, 0.8, 0.9, alpha),
        CelestialBodyId::Neptune => Color::srgba(0.3, 0.5, 0.9, alpha),
        // Moons use gray
        CelestialBodyId::Moon => Color::srgba(0.5, 0.5, 0.5, alpha),
        CelestialBodyId::Io
        | CelestialBodyId::Europa
        | CelestialBodyId::Ganymede
        | CelestialBodyId::Callisto => Color::srgba(0.5, 0.5, 0.5, alpha),
        CelestialBodyId::Titan => Color::srgba(0.5, 0.5, 0.5, alpha),
        // Sun has no orbit
        CelestialBodyId::Sun => Color::NONE,
    }
}

/// Draw orbit paths for planets as aligned idealized Kepler ellipses.
///
/// We use the ephemeris position "now" to align the ellipse so the planet lies on the path.
pub fn draw_orbit_paths(
    mut gizmos: Gizmos,
    settings: Res<OrbitPathSettings>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
) {
    if !settings.visible {
        return;
    }

    let segments = settings.segments.max(64);

    for &id in CelestialBodyId::PLANETS {
        let color = orbit_color(id, settings.alpha);
        if color == Color::NONE {
            continue;
        }

        // Current ephemeris position (source of truth for alignment)
        let Some(pos_now) = ephemeris.get_position_by_id(id, sim_time.current) else {
            continue;
        };
        let r_now = pos_now.length();
        if !r_now.is_finite() || r_now <= 0.0 {
            continue;
        }

        // Base Kepler parameters from the baked orbital elements (used as a stable ellipse shape).
        // This is *rendering only*; the phase/orientation is aligned so the current ephemeris
        // position lies on the curve.
        //
        // IMPORTANT: Do not duplicate constants here. Reuse the canonical ephemeris body data.
        let (a, e_base, omega_base) = all_bodies()
            .into_iter()
            .find(|b| b.id == id)
            .and_then(|b| {
                b.orbit
                    .as_ref()
                    .map(|o| (o.semi_major_axis, o.eccentricity, o.argument_of_periapsis))
            })
            .unwrap_or_else(|| {
                // Should not happen for PLANETS, but keep rendering resilient.
                (r_now, 0.0, 0.0)
            });

        let mut e = e_base * settings.eccentricity_scale;
        e = e.clamp(0.0, 0.999_999);

        // Semi-latus rectum.
        let p = a * (1.0 - e * e);

        // Align the ellipse’s argument-of-periapsis so that the current ephemeris position lies on it.
        //
        // Using polar form from focus:
        //   r = p / (1 + e cos(ν))
        // Solve for cos(ν): cos(ν) = (p/r - 1) / e  (when e > 0)
        // Then choose ν such that the point's inertial angle θ = atan2(y,x) satisfies:
        //   θ = ν + ω  =>  ω = θ - ν
        //
        // This ensures the curve passes through the current planet location.
        let theta_now = pos_now.y.atan2(pos_now.x);
        let omega_aligned = if e > 1e-9 {
            let cos_nu = ((p / r_now) - 1.0) / e;
            let cos_nu = cos_nu.clamp(-1.0, 1.0);
            let nu = cos_nu.acos();
            // Pick the ν branch that yields a better match by comparing reconstructed point direction.
            // (Cheap disambiguation: try ±ν.)
            let omega1 = theta_now - nu;
            let omega2 = theta_now + nu;
            // Keep the aligned ω reasonably close to the base ω to reduce jitter.
            if angle_distance(omega1, omega_base) < angle_distance(omega2, omega_base) {
                omega1
            } else {
                omega2
            }
        } else {
            // Near-circular: orientation is arbitrary; just keep the base orientation.
            omega_base
        };

        // Draw full ellipse as a closed polyline.
        let mut first: Option<Vec3> = None;
        let mut prev: Option<Vec3> = None;

        for i in 0..=segments {
            let nu = (i as f64 / segments as f64) * std::f64::consts::TAU;
            let r = if e > 0.0 { p / (1.0 + e * nu.cos()) } else { a };
            let angle = nu + omega_aligned;

            let x = r * angle.cos();
            let y = r * angle.sin();

            let pt = Vec3::new(
                (x * RENDER_SCALE) as f32,
                (y * RENDER_SCALE) as f32,
                z_layers::TRAJECTORY,
            );

            if first.is_none() {
                first = Some(pt);
            }
            if let Some(p0) = prev {
                // Dashed effect: draw/skip segments in a stable repeating pattern.
                //
                // We base the pattern on the segment index so it doesn't "crawl" as the camera moves
                // or as time changes. The pattern is applied per-planet, starting at i=0.
                let on = settings.dash_on.max(1);
                let off = settings.dash_off;
                let period = on + off;

                let draw = if period == 0 { true } else { (i % period) < on };

                if draw {
                    gizmos.line(p0, pt, color);
                }
            }
            prev = Some(pt);
        }

        if let (Some(last), Some(first)) = (prev, first) {
            gizmos.line(last, first, color);
        }
    }
}

/// Smallest absolute distance between two angles (radians), in [0, π].
fn angle_distance(a: f64, b: f64) -> f64 {
    let mut d = (a - b).rem_euclid(std::f64::consts::TAU);
    if d > std::f64::consts::PI {
        d = std::f64::consts::TAU - d;
    }
    d.abs()
}

// Orbit rendering intentionally uses an idealized Keplerian ellipse (from baked elements),
// aligned to pass through the current ephemeris position. This provides clean, stable visuals
// while ensuring the planet is always on its drawn path at the current time.

/// Draw moon orbit paths centered on their parent planet's current position.
///
/// Moon orbits are scaled radially to match the visual distortion applied to moons,
/// ensuring the orbit path passes through the moon's distorted position.
pub fn draw_moon_orbit_paths(
    mut gizmos: Gizmos,
    settings: Res<OrbitPathSettings>,
    bodies: Query<(&Transform, &CelestialBody, &EffectiveVisualRadius)>,
) {
    if !settings.visible {
        return;
    }

    let segments = settings.segments.max(64);

    // Collect parent positions and radii
    let mut parent_data: std::collections::HashMap<CelestialBodyId, (Vec2, f32)> =
        std::collections::HashMap::new();

    for (transform, body, eff_radius) in bodies.iter() {
        if body.id.parent().is_none() && body.id != CelestialBodyId::Sun {
            // This is a planet
            parent_data.insert(
                body.id,
                (transform.translation.truncate(), eff_radius.0),
            );
        }
    }

    // Draw orbits for each moon
    for &id in CelestialBodyId::MOONS {
        let Some(parent_id) = id.parent() else {
            continue;
        };

        let Some(&(parent_pos, parent_radius)) = parent_data.get(&parent_id) else {
            continue;
        };

        let color = orbit_color(id, settings.alpha * 0.7); // Slightly dimmer for moon orbits
        if color == Color::NONE {
            continue;
        }

        // Find this moon's current distorted position
        let Some((moon_transform, _, moon_eff_radius)) = bodies
            .iter()
            .find(|(_, b, _)| b.id == id)
        else {
            continue;
        };

        let moon_pos = moon_transform.translation.truncate();
        let moon_direction = (moon_pos - parent_pos).normalize_or_zero();
        let moon_distance = (moon_pos - parent_pos).length();

        // Get moon's orbital parameters for shape
        let (a_local, e_base, omega_base) = all_bodies()
            .into_iter()
            .find(|b| b.id == id)
            .and_then(|b| {
                b.orbit
                    .as_ref()
                    .map(|o| (o.semi_major_axis, o.eccentricity, o.argument_of_periapsis))
            })
            .unwrap_or_else(|| (moon_distance as f64 / RENDER_SCALE, 0.0, 0.0));

        // Convert orbital semi-major axis to render units
        let a_render = (a_local * RENDER_SCALE) as f32;

        // Calculate scale factor to match distorted position
        // The orbit should be scaled so the moon's current position lies on it
        let scale_factor = if a_render > 0.001 {
            moon_distance / a_render
        } else {
            1.0
        };

        // Ensure orbit clears the parent's visual radius
        let min_orbit_radius = parent_radius * (1.0 + MARGIN_FRACTION) + moon_eff_radius.0;
        let actual_scale = scale_factor.max(min_orbit_radius / a_render);

        let mut e = e_base * settings.eccentricity_scale;
        e = e.clamp(0.0, 0.999_999);

        // Semi-latus rectum (in local orbital units, before scaling)
        let p_local = a_local * (1.0 - e * e);

        // Align the orbit so the moon's current position lies on it
        let moon_angle = moon_direction.y.atan2(moon_direction.x) as f64;
        let omega_aligned = if e > 1e-9 {
            // For eccentric orbits, solve for the argument of periapsis
            let r_local = moon_distance as f64 / (actual_scale as f64 * RENDER_SCALE);
            let cos_nu = ((p_local / r_local) - 1.0) / e;
            let cos_nu = cos_nu.clamp(-1.0, 1.0);
            let nu = cos_nu.acos();

            let omega1 = moon_angle - nu;
            let omega2 = moon_angle + nu;
            if angle_distance(omega1, omega_base) < angle_distance(omega2, omega_base) {
                omega1
            } else {
                omega2
            }
        } else {
            omega_base
        };

        // Draw the scaled orbit ellipse centered on parent
        let mut first: Option<Vec3> = None;
        let mut prev: Option<Vec3> = None;

        for i in 0..=segments {
            let nu = (i as f64 / segments as f64) * std::f64::consts::TAU;
            let r_local = if e > 0.0 {
                p_local / (1.0 + e * nu.cos())
            } else {
                a_local
            };
            let angle = nu + omega_aligned;

            // Convert to render coordinates, apply scale, center on parent
            let x = (r_local * angle.cos() * RENDER_SCALE) as f32 * actual_scale;
            let y = (r_local * angle.sin() * RENDER_SCALE) as f32 * actual_scale;

            let pt = Vec3::new(
                parent_pos.x + x,
                parent_pos.y + y,
                z_layers::TRAJECTORY,
            );

            if first.is_none() {
                first = Some(pt);
            }
            if let Some(p0) = prev {
                let on = settings.dash_on.max(1);
                let off = settings.dash_off;
                let period = on + off;
                let draw = if period == 0 { true } else { (i % period) < on };

                if draw {
                    gizmos.line(p0, pt, color);
                }
            }
            prev = Some(pt);
        }

        // Close the loop
        if let (Some(last), Some(first)) = (prev, first) {
            let on = settings.dash_on.max(1);
            let off = settings.dash_off;
            let period = on + off;
            let draw = if period == 0 { true } else { (0 % period) < on };
            if draw {
                gizmos.line(last, first, color);
            }
        }
    }
}


/// Settings for Sphere of Influence (SOI) visualization.
#[derive(Resource)]
pub struct SoiSettings {
    /// Whether to show SOI circles.
    pub visible: bool,
    /// Number of gradient rings to draw (more = smoother gradient).
    pub gradient_rings: u32,
    /// Maximum alpha for the innermost ring.
    pub max_alpha: f32,
    /// Number of segments per ring (higher = smoother circles).
    pub segments: u32,
}

impl Default for SoiSettings {
    fn default() -> Self {
        Self {
            visible: true,
            gradient_rings: 8,
            max_alpha: 0.12,
            segments: 48,
        }
    }
}

/// Draw Sphere of Influence (Hill sphere) gradient around planets.
///
/// Shows the region where each planet's gravity dominates over the Sun's.
/// Uses the physical Hill sphere radius (zoom-independent) with a gradient
/// that fades from the planet outward, visualizing the "gravity pull region".
pub fn draw_soi_circles(
    bodies: Query<(&Transform, &CelestialBody)>,
    ephemeris: Res<Ephemeris>,
    settings: Res<SoiSettings>,
    orbit_settings: Res<OrbitPathSettings>,
    mut gizmos: Gizmos,
) {
    if !settings.visible || !orbit_settings.visible {
        return;
    }

    for (transform, body) in bodies.iter() {
        // Only draw SOI for planets (not Sun or moons)
        if !CelestialBodyId::PLANETS.contains(&body.id) {
            continue;
        }

        // Get body data for Hill sphere
        let Some(data) = ephemeris.get_body_data_by_id(body.id) else {
            continue;
        };

        // Skip if no Hill sphere data
        if data.hill_sphere <= 0.0 {
            continue;
        }

        // Use physical Hill sphere scaled to render units (zoom-independent)
        let soi_render_radius = (data.hill_sphere * RENDER_SCALE) as f32;

        // Get base color for this planet (without alpha)
        let (r, g, b) = body_orbit_rgb(body.id);

        // Center position
        let center = Vec3::new(transform.translation.x, transform.translation.y, z_layers::TRAJECTORY);

        // Draw gradient rings: from inner (opaque) to outer (transparent)
        // Gradient represents gravity strength falling off with distance
        for ring in 0..settings.gradient_rings {
            // Ring position: 0 = innermost, gradient_rings-1 = outermost (at Hill sphere)
            let ring_fraction = (ring + 1) as f32 / settings.gradient_rings as f32;
            let ring_radius = soi_render_radius * ring_fraction;

            // Alpha falls off towards the edge (stronger gravity = more visible)
            // Use quadratic falloff to emphasize the inner region
            let alpha = settings.max_alpha * (1.0 - ring_fraction * ring_fraction);
            let color = Color::srgba(r, g, b, alpha);

            // Draw circle at this radius
            draw_circle_ring(&mut gizmos, center, ring_radius, settings.segments, color);
        }
    }
}

/// Draw a single circle ring using line segments.
fn draw_circle_ring(gizmos: &mut Gizmos, center: Vec3, radius: f32, segments: u32, color: Color) {
    let mut prev: Option<Vec3> = None;
    let mut first: Option<Vec3> = None;

    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let pt = Vec3::new(
            center.x + radius * angle.cos(),
            center.y + radius * angle.sin(),
            center.z,
        );

        if first.is_none() {
            first = Some(pt);
        }

        if let Some(p0) = prev {
            gizmos.line(p0, pt, color);
        }

        prev = Some(pt);
    }
}

/// Get RGB color for a body's orbit/SOI.
fn body_orbit_rgb(body_id: CelestialBodyId) -> (f32, f32, f32) {
    match body_id {
        CelestialBodyId::Mercury => (0.7, 0.7, 0.75),
        CelestialBodyId::Venus => (0.9, 0.85, 0.6),
        CelestialBodyId::Earth => (0.3, 0.6, 1.0),
        CelestialBodyId::Mars => (1.0, 0.5, 0.3),
        CelestialBodyId::Jupiter => (1.0, 0.8, 0.5),
        CelestialBodyId::Saturn => (0.9, 0.85, 0.6),
        CelestialBodyId::Uranus => (0.6, 0.9, 0.9),
        CelestialBodyId::Neptune => (0.4, 0.5, 1.0),
        _ => (0.5, 0.5, 0.5),
    }
}


/// Settings for danger zone visualization (collision detection radius).
#[derive(Resource)]
pub struct DangerZoneSettings {
    /// Whether to show danger zone rings.
    pub visible: bool,
    /// Alpha value for the danger zone ring.
    pub alpha: f32,
    /// Number of segments per ring (higher = smoother circles).
    pub segments: u32,
}

impl Default for DangerZoneSettings {
    fn default() -> Self {
        Self {
            visible: true,
            alpha: 0.15,
            segments: 64,
        }
    }
}

/// Draw danger zone rings around planets showing collision detection radius.
///
/// These rings visualize the `COLLISION_MULTIPLIER` radius used for collision
/// detection, making it clear to players where asteroids will be considered
/// as "hitting" a planet.
pub fn draw_danger_zones(
    bodies: Query<(&Transform, &CelestialBody)>,
    ephemeris: Res<Ephemeris>,
    settings: Res<DangerZoneSettings>,
    mut gizmos: Gizmos,
) {
    if !settings.visible {
        return;
    }

    for (transform, body) in bodies.iter() {
        // Only draw danger zones for planets (not Sun or moons)
        if !CelestialBodyId::PLANETS.contains(&body.id) {
            continue;
        }

        // Get body data for radius
        let Some(data) = ephemeris.get_body_data_by_id(body.id) else {
            continue;
        };

        // Calculate danger zone radius using COLLISION_MULTIPLIER
        use crate::ephemeris::COLLISION_MULTIPLIER;
        let danger_radius = (data.radius * COLLISION_MULTIPLIER * RENDER_SCALE) as f32;

        // Use a red-tinted color for danger zones
        let color = Color::srgba(1.0, 0.3, 0.3, settings.alpha);

        // Center position
        let center = Vec3::new(
            transform.translation.x,
            transform.translation.y,
            z_layers::TRAJECTORY - 0.1, // Slightly behind trajectories
        );

        // Draw the danger zone ring
        draw_circle_ring(&mut gizmos, center, danger_radius, settings.segments, color);
    }
}
