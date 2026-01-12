//! Orbit path rendering using Bevy Gizmos.
//!
//! Draws elliptical orbit paths for celestial bodies.

use std::f32::consts::TAU;

use bevy::prelude::*;

use crate::camera::RENDER_SCALE;
use crate::ephemeris::data::{all_bodies, CelestialBodyId};
use crate::render::z_layers;

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
    /// Number of segments for drawing ellipses.
    pub segments: u32,
    /// Alpha value for orbit path color.
    pub alpha: f32,
}

impl Default for OrbitPathSettings {
    fn default() -> Self {
        Self {
            visible: true,
            segments: 128,
            alpha: 0.3,
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
        CelestialBodyId::Io | CelestialBodyId::Europa |
        CelestialBodyId::Ganymede | CelestialBodyId::Callisto => Color::srgba(0.5, 0.5, 0.5, alpha),
        CelestialBodyId::Titan => Color::srgba(0.5, 0.5, 0.5, alpha),
        // Sun has no orbit
        CelestialBodyId::Sun => Color::NONE,
    }
}

/// Draw orbit paths for all celestial bodies with orbits.
///
/// Uses the same polar form as the Kepler solver to ensure planets appear on their orbits:
/// - r = a(1 - e²) / (1 + e * cos(ν))  where ν is true anomaly
/// - position = (r * cos(ν + ω), r * sin(ν + ω))  where ω is argument of periapsis
fn draw_orbit_paths(
    mut gizmos: Gizmos,
    settings: Res<OrbitPathSettings>,
) {
    if !settings.visible {
        return;
    }

    for body in all_bodies() {
        let Some(orbit) = &body.orbit else {
            continue; // Skip bodies without orbits (Sun)
        };

        // Skip moons for now - their orbits are too small to see at solar system scale
        if matches!(body.id,
            CelestialBodyId::Moon |
            CelestialBodyId::Io | CelestialBodyId::Europa |
            CelestialBodyId::Ganymede | CelestialBodyId::Callisto |
            CelestialBodyId::Titan
        ) {
            continue;
        }

        let color = orbit_color(body.id, settings.alpha);
        if color == Color::NONE {
            continue;
        }

        // Orbital elements in render scale
        let a = (orbit.semi_major_axis * RENDER_SCALE) as f32;
        let e = orbit.eccentricity as f32;
        let omega = orbit.argument_of_periapsis as f32;

        // Semi-latus rectum: p = a(1 - e²)
        let p = a * (1.0 - e * e);

        // Draw ellipse as dashed line segments using polar form from focus
        let segments = settings.segments;
        for i in 0..segments {
            // Skip every other segment for dashed effect
            if i % 2 != 0 {
                continue;
            }

            // True anomaly values for this segment
            let nu0 = (i as f32 / segments as f32) * TAU;
            let nu1 = ((i + 1) as f32 / segments as f32) * TAU;

            // Radius from focus using polar form: r = p / (1 + e * cos(ν))
            let r0 = p / (1.0 + e * nu0.cos());
            let r1 = p / (1.0 + e * nu1.cos());

            // Final angle = true anomaly + argument of periapsis
            // This matches exactly what the Kepler solver computes
            let angle0 = nu0 + omega;
            let angle1 = nu1 + omega;

            // Position in orbital plane (Sun at origin)
            let x0 = r0 * angle0.cos();
            let y0 = r0 * angle0.sin();
            let x1 = r1 * angle1.cos();
            let y1 = r1 * angle1.sin();

            let p0 = Vec3::new(x0, y0, z_layers::TRAJECTORY);
            let p1 = Vec3::new(x1, y1, z_layers::TRAJECTORY);

            gizmos.line(p0, p1, color);
        }
    }
}
