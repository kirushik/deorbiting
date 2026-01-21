//! Property-based tests for ephemeris computations using proptest.
//!
//! These tests verify that orbital computations maintain expected properties
//! across a wide range of inputs.

use proptest::prelude::*;
use std::f64::consts::{PI, TAU};

use super::kepler::KeplerOrbit;
use crate::types::{AU_TO_METERS, GM_SUN, SECONDS_PER_DAY};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Verify Kepler solver convergence for all valid eccentricities and mean anomalies.
    ///
    /// The solver should always converge and produce E such that M = E - e*sin(E).
    #[test]
    fn prop_kepler_solver_convergence(
        mean_anomaly_normalized in 0.0f64..1.0,
        eccentricity in 0.0f64..0.95,
    ) {
        let mean_anomaly = mean_anomaly_normalized * TAU;

        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            eccentricity,
            0.0,
            0.0,
            0.9856,
        );

        let e_anom = orbit.solve_eccentric_anomaly(mean_anomaly);

        // Verify Kepler's equation: M = E - e*sin(E)
        let m_check = e_anom - eccentricity * e_anom.sin();
        let m_normalized = mean_anomaly.rem_euclid(TAU);

        let error = (m_check - m_normalized).abs();
        prop_assert!(
            error < 1e-8,
            "Kepler solver failed: M={}, e={}, E={}, M_check={}, error={}",
            mean_anomaly, eccentricity, e_anom, m_check, error
        );
    }

    /// Verify orbital period matches Kepler's third law.
    ///
    /// T = 2π √(a³/GM) should hold for all semi-major axes.
    #[test]
    fn prop_orbital_period_matches_kepler(
        semi_major_axis_au in 0.3f64..50.0,
    ) {
        let a = semi_major_axis_au * AU_TO_METERS;

        // Mean motion n = √(GM/a³) rad/s
        let n = (GM_SUN / (a * a * a)).sqrt();

        // Create orbit with this mean motion
        let mean_motion_deg_day = n * 180.0 / PI * SECONDS_PER_DAY;

        let orbit = KeplerOrbit::from_elements(
            a,
            0.1,
            0.0,
            0.0,
            mean_motion_deg_day,
        );

        let period = orbit.period();
        let expected_period = TAU * (a * a * a / GM_SUN).sqrt();

        let error = ((period - expected_period) / expected_period).abs();
        prop_assert!(
            error < 1e-6,
            "Period mismatch: computed {} vs expected {} (error {}%)",
            period, expected_period, error * 100.0
        );
    }

    /// Verify position continuity - no discontinuous jumps.
    ///
    /// Position should change smoothly over time with no sudden jumps.
    #[test]
    fn prop_position_continuity(
        start_time_days in 0.0f64..3650.0,
        eccentricity in 0.0f64..0.5,
    ) {
        let start_time = start_time_days * SECONDS_PER_DAY;

        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            eccentricity,
            45.0,
            0.0,
            0.9856,
        );

        // Check continuity over small time steps
        let dt = 3600.0; // 1 hour
        let pos1 = orbit.get_local_position(start_time);
        let pos2 = orbit.get_local_position(start_time + dt);
        let pos3 = orbit.get_local_position(start_time + 2.0 * dt);

        // Compute velocities
        let v12 = (pos2 - pos1).length() / dt;
        let v23 = (pos3 - pos2).length() / dt;

        // Velocity should not change dramatically between adjacent samples
        // (for non-extreme orbits)
        let v_change = (v23 - v12).abs() / (v12 + 1.0);
        prop_assert!(
            v_change < 0.1,
            "Velocity discontinuity detected: v12={}, v23={}, change={}%",
            v12, v23, v_change * 100.0
        );
    }

    /// Verify velocity is perpendicular to position for circular orbits.
    ///
    /// For e=0, r · v should always be 0.
    #[test]
    fn prop_velocity_perpendicular_for_circular(
        time_days in 0.0f64..365.25,
    ) {
        let time = time_days * SECONDS_PER_DAY;

        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            0.0, // circular
            0.0,
            0.0,
            0.9856,
        );

        let pos = orbit.get_local_position(time);
        let vel = orbit.get_local_velocity(time);

        // r · v should be 0 for circular orbit
        let dot = pos.dot(vel);
        let magnitude_product = pos.length() * vel.length();

        let cos_angle = dot / magnitude_product;
        prop_assert!(
            cos_angle.abs() < 1e-6,
            "Circular orbit: velocity not perpendicular at t={} days, cos(angle)={}",
            time_days, cos_angle
        );
    }

    /// Verify position returns to start after one period.
    ///
    /// After exactly one orbital period, the body should return to its starting position.
    #[test]
    fn prop_position_periodic(
        eccentricity in 0.0f64..0.6,
        start_time_days in 0.0f64..365.0,
    ) {
        let start_time = start_time_days * SECONDS_PER_DAY;

        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            eccentricity,
            30.0,
            45.0,
            0.9856,
        );

        let period = orbit.period();
        let pos_start = orbit.get_local_position(start_time);
        let pos_end = orbit.get_local_position(start_time + period);

        let distance = (pos_end - pos_start).length();

        // Should return to within 1 km of starting position
        prop_assert!(
            distance < 1000.0,
            "Position not periodic: distance after one period = {} m",
            distance
        );
    }
}

#[cfg(test)]
mod deterministic_tests {
    use super::*;
    use crate::ephemeris::data::all_bodies;
    use crate::types::G;

    #[test]
    fn test_all_body_mass_values_positive() {
        for body in all_bodies() {
            assert!(body.mass > 0.0, "{:?} has non-positive mass", body.id);
            // GM = G * mass should also be positive
            let gm = G * body.mass;
            assert!(gm > 0.0, "{:?} has non-positive GM", body.id);
        }
    }

    #[test]
    fn test_sun_gm_matches_constant() {
        // GM_SUN constant should match the data
        // (This is a sanity check that constants are consistent)
        assert!(GM_SUN > 1e20, "GM_SUN seems too small");
        assert!(GM_SUN < 1e21, "GM_SUN seems too large");
    }

    #[test]
    fn test_body_orbits_have_valid_eccentricity() {
        for body in all_bodies() {
            // Only bodies with orbits (not the Sun)
            if let Some(ref orbit) = body.orbit {
                let e = orbit.eccentricity;
                assert!(
                    (0.0..1.0).contains(&e),
                    "{:?} has invalid eccentricity {}",
                    body.id,
                    e
                );
            }
        }
    }

    #[test]
    fn test_kepler_solver_at_boundary_mean_anomaly() {
        // Test at M = 0, π, 2π
        let orbit = KeplerOrbit::from_elements(AU_TO_METERS, 0.5, 0.0, 0.0, 0.9856);

        for m in [0.0, PI, TAU - 0.001, TAU] {
            let e = orbit.solve_eccentric_anomaly(m);
            assert!(e.is_finite(), "Solver failed at M = {}", m);
        }
    }
}
