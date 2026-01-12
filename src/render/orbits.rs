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
use crate::ephemeris::{CelestialBodyId, Ephemeris, all_bodies};
use crate::render::z_layers;
use crate::types::SimulationTime;

/// Plugin providing orbit path visualization.
pub struct OrbitPathPlugin;

impl Plugin for OrbitPathPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OrbitPathSettings>()
            .add_systems(Update, draw_orbit_paths);
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
fn draw_orbit_paths(
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
