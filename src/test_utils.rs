//! Test utilities for orbital mechanics simulation tests.
//!
//! Provides fixtures for creating test orbits and assertions for verifying
//! physical invariants like energy and angular momentum conservation.

use bevy::math::DVec2;

use crate::types::{BodyState, AU_TO_METERS, GM_SUN};

/// Fixtures for creating test orbital states.
pub mod fixtures {
    use super::*;

    /// Create a body in a circular orbit at the given distance from the Sun.
    ///
    /// The body is placed on the positive x-axis with velocity in the +y direction.
    pub fn circular_orbit(distance_au: f64) -> BodyState {
        let r = distance_au * AU_TO_METERS;
        // Circular orbit velocity: v = sqrt(GM/r)
        let v = (GM_SUN / r).sqrt();
        BodyState {
            pos: DVec2::new(r, 0.0),
            vel: DVec2::new(0.0, v),
            mass: 1.0, // Test mass, doesn't affect orbital dynamics
        }
    }

    /// Create a body in an elliptical orbit at perihelion.
    ///
    /// The body starts at perihelion (closest approach) on the positive x-axis.
    pub fn elliptical_orbit(perihelion_au: f64, eccentricity: f64) -> BodyState {
        assert!(
            (0.0..1.0).contains(&eccentricity),
            "Eccentricity must be in [0, 1) for elliptical orbit"
        );

        let r_p = perihelion_au * AU_TO_METERS;
        // Semi-major axis from perihelion and eccentricity: a = r_p / (1 - e)
        let a = r_p / (1.0 - eccentricity);
        // Vis-viva equation at perihelion: v = sqrt(GM * (2/r - 1/a))
        let v = (GM_SUN * (2.0 / r_p - 1.0 / a)).sqrt();

        BodyState {
            pos: DVec2::new(r_p, 0.0),
            vel: DVec2::new(0.0, v),
            mass: 1.0,
        }
    }

    /// Create a body on an escape trajectory (hyperbolic orbit).
    ///
    /// The body starts at the given distance with velocity greater than escape velocity.
    pub fn escape_trajectory(distance_au: f64) -> BodyState {
        let r = distance_au * AU_TO_METERS;
        // Escape velocity: v_esc = sqrt(2 * GM / r)
        // Use 1.1x escape velocity to ensure hyperbolic trajectory
        let v_esc = (2.0 * GM_SUN / r).sqrt();
        let v = v_esc * 1.1;

        BodyState {
            pos: DVec2::new(r, 0.0),
            vel: DVec2::new(0.0, v),
            mass: 1.0,
        }
    }

    /// Create a body with specific orbital energy.
    ///
    /// Positive energy = hyperbolic (escape), negative = elliptical, zero = parabolic.
    pub fn orbit_with_energy(distance_au: f64, specific_energy: f64) -> BodyState {
        let r = distance_au * AU_TO_METERS;
        // Specific energy: E = v²/2 - GM/r
        // Solving for v: v = sqrt(2 * (E + GM/r))
        let v_squared = 2.0 * (specific_energy + GM_SUN / r);
        assert!(v_squared >= 0.0, "Energy too low for this distance");
        let v = v_squared.sqrt();

        BodyState {
            pos: DVec2::new(r, 0.0),
            vel: DVec2::new(0.0, v),
            mass: 1.0,
        }
    }
}

/// Assertions for verifying physical invariants.
pub mod assertions {
    use super::*;

    /// Compute specific orbital energy (energy per unit mass).
    ///
    /// E = v²/2 - GM/r
    /// Negative for bound orbits, zero for parabolic, positive for hyperbolic.
    pub fn orbital_energy(pos: DVec2, vel: DVec2) -> f64 {
        let r = pos.length();
        let v = vel.length();
        0.5 * v * v - GM_SUN / r
    }

    /// Compute specific angular momentum (2D scalar).
    ///
    /// L = r × v (z-component of cross product in 2D)
    pub fn angular_momentum(pos: DVec2, vel: DVec2) -> f64 {
        pos.x * vel.y - pos.y * vel.x
    }

    /// Assert that energy is conserved within tolerance.
    ///
    /// # Panics
    /// Panics if relative energy drift exceeds tolerance.
    pub fn assert_energy_conserved(initial_energy: f64, final_energy: f64, tolerance: f64) {
        let drift = if initial_energy.abs() > 1e-10 {
            ((final_energy - initial_energy) / initial_energy).abs()
        } else {
            (final_energy - initial_energy).abs()
        };
        assert!(
            drift <= tolerance,
            "Energy not conserved: initial={initial_energy:.6e}, final={final_energy:.6e}, drift={drift:.6e}, tolerance={tolerance:.6e}"
        );
    }

    /// Assert that angular momentum is conserved within tolerance.
    ///
    /// # Panics
    /// Panics if relative angular momentum drift exceeds tolerance.
    pub fn assert_angular_momentum_conserved(initial_l: f64, final_l: f64, tolerance: f64) {
        let drift = if initial_l.abs() > 1e-10 {
            ((final_l - initial_l) / initial_l).abs()
        } else {
            (final_l - initial_l).abs()
        };
        assert!(
            drift <= tolerance,
            "Angular momentum not conserved: initial={initial_l:.6e}, final={final_l:.6e}, drift={drift:.6e}, tolerance={tolerance:.6e}"
        );
    }

    /// Compute orbital period for an elliptical orbit.
    ///
    /// Uses Kepler's third law: T = 2π * sqrt(a³/GM)
    pub fn orbital_period(semi_major_axis: f64) -> f64 {
        use std::f64::consts::TAU;
        TAU * (semi_major_axis.powi(3) / GM_SUN).sqrt()
    }

    /// Compute semi-major axis from orbital energy.
    ///
    /// For bound orbits: a = -GM / (2E)
    pub fn semi_major_axis_from_energy(energy: f64) -> Option<f64> {
        if energy >= 0.0 {
            None // Unbound orbit
        } else {
            Some(-GM_SUN / (2.0 * energy))
        }
    }

    /// Check if an orbit is bound (elliptical) or unbound (hyperbolic/parabolic).
    pub fn is_bound(pos: DVec2, vel: DVec2) -> bool {
        orbital_energy(pos, vel) < 0.0
    }

    /// Compute escape velocity at a given distance.
    pub fn escape_velocity(distance: f64) -> f64 {
        (2.0 * GM_SUN / distance).sqrt()
    }
}

/// Utilities for creating headless Bevy apps for testing.
pub mod bevy_test {
    use bevy::prelude::*;

    /// Create a minimal Bevy app for testing without rendering.
    ///
    /// This app uses MinimalPlugins for a lightweight test environment.
    pub fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_circular_orbit_has_correct_velocity() {
        let state = fixtures::circular_orbit(1.0); // 1 AU
        let expected_v = (GM_SUN / AU_TO_METERS).sqrt();
        assert_relative_eq!(state.vel.length(), expected_v, epsilon = 1.0);
    }

    #[test]
    fn test_circular_orbit_is_bound() {
        let state = fixtures::circular_orbit(1.0);
        assert!(assertions::is_bound(state.pos, state.vel));
    }

    #[test]
    fn test_escape_trajectory_is_unbound() {
        let state = fixtures::escape_trajectory(1.0);
        assert!(!assertions::is_bound(state.pos, state.vel));
    }

    #[test]
    fn test_elliptical_orbit_energy() {
        let state = fixtures::elliptical_orbit(1.0, 0.5);
        let energy = assertions::orbital_energy(state.pos, state.vel);
        assert!(energy < 0.0, "Elliptical orbit should have negative energy");
    }

    #[test]
    fn test_angular_momentum_perpendicular() {
        let state = fixtures::circular_orbit(1.0);
        let l = assertions::angular_momentum(state.pos, state.vel);
        // For circular orbit starting at (r, 0) with vel (0, v), L = r * v
        let expected_l = state.pos.length() * state.vel.length();
        assert_relative_eq!(l, expected_l, epsilon = 1.0);
    }

    #[test]
    fn test_orbital_period_earth() {
        // Earth's semi-major axis is 1 AU
        let period = assertions::orbital_period(AU_TO_METERS);
        // Should be approximately 1 year in seconds
        let year_seconds = 365.25 * 24.0 * 3600.0;
        assert_relative_eq!(period, year_seconds, epsilon = year_seconds * 0.01);
    }
}
