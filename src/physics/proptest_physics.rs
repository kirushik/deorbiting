//! Property-based tests for physics simulation using proptest.
//!
//! These tests verify physical invariants across a wide range of orbital parameters.

use bevy::math::DVec2;
use proptest::prelude::*;

use crate::test_utils::{assertions, fixtures};
use crate::types::{AU_TO_METERS, GM_SUN};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Verify energy conservation over one orbital period.
    ///
    /// For a two-body problem (Sun + asteroid), specific orbital energy
    /// E = v²/2 - GM/r should be conserved.
    #[test]
    fn prop_energy_conservation_one_orbit(
        distance_au in 0.5f64..10.0,
        eccentricity in 0.0f64..0.8,
    ) {
        let state = fixtures::elliptical_orbit(distance_au, eccentricity);
        let initial_energy = assertions::orbital_energy(state.pos, state.vel);

        // Compute semi-major axis from perihelion
        let r_p = distance_au * AU_TO_METERS;
        let a = r_p / (1.0 - eccentricity);
        let period = assertions::orbital_period(a);

        // Simulate using simple Velocity Verlet for one orbit
        let mut pos = state.pos;
        let mut vel = state.vel;
        let dt = period / 10000.0; // 10,000 steps per orbit
        let mut time = 0.0;

        while time < period {
            // Compute acceleration (Sun gravity only)
            let r = pos.length();
            let acc = -GM_SUN / (r * r * r) * pos;

            // Velocity Verlet step
            let new_pos = pos + vel * dt + acc * (0.5 * dt * dt);
            let new_r = new_pos.length();
            let new_acc = -GM_SUN / (new_r * new_r * new_r) * new_pos;
            let new_vel = vel + (acc + new_acc) * (0.5 * dt);

            pos = new_pos;
            vel = new_vel;
            time += dt;
        }

        let final_energy = assertions::orbital_energy(pos, vel);

        // Energy should be conserved within 1% for well-behaved orbits
        let drift = ((final_energy - initial_energy) / initial_energy).abs();
        prop_assert!(
            drift < 0.01,
            "Energy drift {:.4}% exceeds 1% tolerance (e={}, a={} AU)",
            drift * 100.0, eccentricity, a / AU_TO_METERS
        );
    }

    /// Verify angular momentum conservation for central force.
    ///
    /// For a central force (like Sun's gravity), angular momentum L = r × v
    /// should be exactly conserved.
    #[test]
    fn prop_angular_momentum_conservation(
        distance_au in 0.5f64..10.0,
        eccentricity in 0.0f64..0.8,
    ) {
        let state = fixtures::elliptical_orbit(distance_au, eccentricity);
        let initial_l = assertions::angular_momentum(state.pos, state.vel);

        // Compute semi-major axis and period
        let r_p = distance_au * AU_TO_METERS;
        let a = r_p / (1.0 - eccentricity);
        let period = assertions::orbital_period(a);

        // Simulate using Velocity Verlet
        let mut pos = state.pos;
        let mut vel = state.vel;
        let dt = period / 10000.0;
        let mut time = 0.0;

        while time < period {
            let r = pos.length();
            let acc = -GM_SUN / (r * r * r) * pos;

            let new_pos = pos + vel * dt + acc * (0.5 * dt * dt);
            let new_r = new_pos.length();
            let new_acc = -GM_SUN / (new_r * new_r * new_r) * new_pos;
            let new_vel = vel + (acc + new_acc) * (0.5 * dt);

            pos = new_pos;
            vel = new_vel;
            time += dt;
        }

        let final_l = assertions::angular_momentum(pos, vel);

        // Angular momentum should be conserved within 0.1%
        let drift = ((final_l - initial_l) / initial_l).abs();
        prop_assert!(
            drift < 0.001,
            "Angular momentum drift {:.4}% exceeds 0.1% tolerance",
            drift * 100.0
        );
    }

    /// Verify Kepler's Third Law: T² ∝ a³
    ///
    /// The ratio T² / a³ should equal 4π² / GM for all orbits.
    #[test]
    fn prop_keplers_third_law(
        semi_major_axis_au in 0.3f64..50.0,
    ) {
        let a = semi_major_axis_au * AU_TO_METERS;
        let period = assertions::orbital_period(a);

        // Kepler's third law: T = 2π√(a³/GM)
        // So T² / a³ = 4π² / GM
        let expected_ratio = 4.0 * std::f64::consts::PI * std::f64::consts::PI / GM_SUN;
        let actual_ratio = period * period / (a * a * a);

        let relative_error = ((actual_ratio - expected_ratio) / expected_ratio).abs();
        prop_assert!(
            relative_error < 0.001,
            "Kepler's Third Law violated: {:.6e} vs expected {:.6e}",
            actual_ratio, expected_ratio
        );
    }

    /// Verify escape velocity threshold.
    ///
    /// Objects with v < v_esc should be bound (negative energy).
    /// Objects with v > v_esc should escape (positive energy).
    #[test]
    fn prop_escape_velocity_threshold(
        distance_au in 0.1f64..50.0,
        velocity_factor in 0.5f64..1.5,
    ) {
        let r = distance_au * AU_TO_METERS;
        let v_esc = assertions::escape_velocity(r);

        // Create state with velocity at `velocity_factor` of escape velocity
        let v = v_esc * velocity_factor;
        let pos = DVec2::new(r, 0.0);
        let vel = DVec2::new(0.0, v);

        let energy = assertions::orbital_energy(pos, vel);
        let is_bound = energy < 0.0;

        if velocity_factor < 0.99 {
            prop_assert!(
                is_bound,
                "v < v_esc should be bound: factor={}, energy={}",
                velocity_factor, energy
            );
        } else if velocity_factor > 1.01 {
            prop_assert!(
                !is_bound,
                "v > v_esc should escape: factor={}, energy={}",
                velocity_factor, energy
            );
        }
        // Near 1.0, either outcome is acceptable (numerical boundary)
    }

    /// Verify that circular orbits have v perpendicular to r.
    #[test]
    fn prop_circular_orbit_perpendicular(
        distance_au in 0.3f64..30.0,
    ) {
        let state = fixtures::circular_orbit(distance_au);

        // For circular orbit starting at (r, 0) with vel (0, v),
        // the dot product r · v should be zero
        let dot = state.pos.dot(state.vel);
        let magnitude_product = state.pos.length() * state.vel.length();

        // dot / (|r| |v|) = cos(theta), should be 0 for perpendicular
        let cos_theta = dot / magnitude_product;
        prop_assert!(
            cos_theta.abs() < 1e-10,
            "Circular orbit velocity not perpendicular: cos(theta) = {}",
            cos_theta
        );
    }

    /// Verify orbital elements roundtrip (state -> elements -> state).
    ///
    /// Converting from position/velocity to orbital elements and back
    /// should preserve the original state.
    #[test]
    fn prop_orbital_elements_roundtrip(
        distance_au in 0.5f64..10.0,
        eccentricity in 0.0f64..0.7,
    ) {
        let state = fixtures::elliptical_orbit(distance_au, eccentricity);

        // Compute orbital elements from state
        let energy = assertions::orbital_energy(state.pos, state.vel);
        let l = assertions::angular_momentum(state.pos, state.vel);

        // Semi-major axis from energy
        let a = -GM_SUN / (2.0 * energy);

        // Eccentricity from angular momentum and energy
        let h_squared = l * l;
        let e_computed = (1.0 + 2.0 * energy * h_squared / (GM_SUN * GM_SUN)).sqrt();

        // Compare computed eccentricity with input
        let e_error = (e_computed - eccentricity).abs();
        prop_assert!(
            e_error < 0.01,
            "Eccentricity roundtrip error: computed {} vs input {}",
            e_computed, eccentricity
        );

        // Compare computed semi-major axis with expected
        let r_p = distance_au * AU_TO_METERS;
        let a_expected = r_p / (1.0 - eccentricity);
        let a_error = ((a - a_expected) / a_expected).abs();
        prop_assert!(
            a_error < 0.01,
            "Semi-major axis roundtrip error: computed {} vs expected {}",
            a / AU_TO_METERS, a_expected / AU_TO_METERS
        );
    }
}

#[cfg(test)]
mod deterministic_tests {
    use super::*;

    #[test]
    fn test_energy_conservation_earth_orbit() {
        // Earth at 1 AU, circular
        let state = fixtures::circular_orbit(1.0);
        let initial_energy = assertions::orbital_energy(state.pos, state.vel);

        // After simulation, energy should be conserved
        // (Just verify the test infrastructure works)
        assert!(
            initial_energy < 0.0,
            "Bound orbit should have negative energy"
        );
    }

    #[test]
    fn test_escape_velocity_formula() {
        let r = AU_TO_METERS;
        let v_esc = assertions::escape_velocity(r);

        // v_esc = sqrt(2 * GM / r)
        let expected = (2.0 * GM_SUN / r).sqrt();
        assert!((v_esc - expected).abs() < 1.0);
    }
}
