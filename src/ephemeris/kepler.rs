//! Kepler orbit solver using Newton's method for the Kepler equation.

use bevy::math::DVec2;
use crate::types::{DEG_TO_RAD, SECONDS_PER_DAY};

/// Keplerian orbital elements for computing positions analytically.
/// All angular values in radians, distances in meters, time in seconds.
#[derive(Clone, Debug)]
pub struct KeplerOrbit {
    /// Semi-major axis in meters
    pub semi_major_axis: f64,
    /// Eccentricity (dimensionless, 0 ≤ e < 1 for ellipse)
    pub eccentricity: f64,
    /// Argument of periapsis in radians
    pub argument_of_periapsis: f64,
    /// Mean anomaly at J2000 epoch in radians
    pub mean_anomaly_at_epoch: f64,
    /// Mean motion in radians per second
    pub mean_motion: f64,
}

impl KeplerOrbit {
    /// Create a new Kepler orbit from orbital elements.
    ///
    /// # Arguments
    /// * `semi_major_axis` - Semi-major axis in meters
    /// * `eccentricity` - Orbital eccentricity (0-1 for elliptical orbits)
    /// * `argument_of_periapsis_deg` - Argument of periapsis in degrees
    /// * `mean_anomaly_at_epoch_deg` - Mean anomaly at J2000 epoch in degrees
    /// * `mean_motion_deg_per_day` - Mean motion in degrees per day
    pub fn from_elements(
        semi_major_axis: f64,
        eccentricity: f64,
        argument_of_periapsis_deg: f64,
        mean_anomaly_at_epoch_deg: f64,
        mean_motion_deg_per_day: f64,
    ) -> Self {
        Self {
            semi_major_axis,
            eccentricity,
            argument_of_periapsis: argument_of_periapsis_deg * DEG_TO_RAD,
            mean_anomaly_at_epoch: mean_anomaly_at_epoch_deg * DEG_TO_RAD,
            mean_motion: mean_motion_deg_per_day * DEG_TO_RAD / SECONDS_PER_DAY,
        }
    }

    /// Solve Kepler's equation M = E - e*sin(E) for eccentric anomaly E
    /// using Newton's method.
    ///
    /// # Arguments
    /// * `mean_anomaly` - Mean anomaly M in radians
    ///
    /// # Returns
    /// Eccentric anomaly E in radians
    ///
    /// # Robustness
    /// Current implementation uses simple Newton's method which works well for
    /// eccentricities up to ~0.95. For extremely high eccentricities (e > 0.97)
    /// or near-parabolic orbits, Newton's method may converge slowly or oscillate.
    ///
    /// TODO: Add fallback to bisection or Halley's method if Newton fails to
    /// converge within iteration limit. Consider returning Result<f64, KeplerError>
    /// to handle convergence failures gracefully.
    pub fn solve_eccentric_anomaly(&self, mean_anomaly: f64) -> f64 {
        // Normalize mean anomaly to [0, 2π)
        let m = mean_anomaly.rem_euclid(std::f64::consts::TAU);

        // Initial guess: E = M for low eccentricity, π for high e
        // TODO: Use more sophisticated initial guess for high eccentricity
        // (e.g., Markley's or Mikkola's starting value)
        let mut e_anomaly = if self.eccentricity < 0.8 {
            m
        } else {
            std::f64::consts::PI
        };

        // Newton's method iteration
        // TODO: Track convergence and fall back to bisection if oscillating
        for _ in 0..50 {
            let sin_e = e_anomaly.sin();
            let cos_e = e_anomaly.cos();

            // f(E) = E - e*sin(E) - M
            let f = e_anomaly - self.eccentricity * sin_e - m;
            // f'(E) = 1 - e*cos(E)
            let f_prime = 1.0 - self.eccentricity * cos_e;

            // Newton step
            let delta = f / f_prime;
            e_anomaly -= delta;

            if delta.abs() < 1e-12 {
                break;
            }
        }

        e_anomaly
    }

    /// Compute true anomaly from eccentric anomaly.
    ///
    /// # Arguments
    /// * `eccentric_anomaly` - Eccentric anomaly E in radians
    ///
    /// # Returns
    /// True anomaly ν in radians
    pub fn eccentric_to_true_anomaly(&self, eccentric_anomaly: f64) -> f64 {
        let e = self.eccentricity;
        let half_e = eccentric_anomaly / 2.0;

        // Using atan2 for full quadrant coverage (atan only returns [-π/2, π/2])
        // Formula: ν = 2 * atan2(sqrt(1+e) * sin(E/2), sqrt(1-e) * cos(E/2))
        let y = (1.0 + e).sqrt() * half_e.sin();
        let x = (1.0 - e).sqrt() * half_e.cos();
        2.0 * y.atan2(x)
    }

    /// Compute orbital radius from eccentric anomaly.
    ///
    /// # Arguments
    /// * `eccentric_anomaly` - Eccentric anomaly E in radians
    ///
    /// # Returns
    /// Distance from focus in meters
    pub fn radius(&self, eccentric_anomaly: f64) -> f64 {
        self.semi_major_axis * (1.0 - self.eccentricity * eccentric_anomaly.cos())
    }

    /// Get position relative to parent body at given time.
    ///
    /// # Arguments
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Position vector (x, y) in meters, in the orbital plane
    pub fn get_local_position(&self, time: f64) -> DVec2 {
        // Mean anomaly at time t
        let mean_anomaly = self.mean_anomaly_at_epoch + self.mean_motion * time;

        // Solve for eccentric anomaly
        let e_anomaly = self.solve_eccentric_anomaly(mean_anomaly);

        // True anomaly
        let true_anomaly = self.eccentric_to_true_anomaly(e_anomaly);

        // Radius
        let radius = self.radius(e_anomaly);

        // Position (rotated by argument of periapsis)
        let angle = true_anomaly + self.argument_of_periapsis;
        DVec2::new(radius * angle.cos(), radius * angle.sin())
    }

    /// Get orbital velocity at given time.
    ///
    /// # Arguments
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Velocity vector (vx, vy) in m/s, in the orbital plane
    pub fn get_local_velocity(&self, time: f64) -> DVec2 {
        // Mean anomaly at time t
        let mean_anomaly = self.mean_anomaly_at_epoch + self.mean_motion * time;

        // Solve for eccentric anomaly
        let e_anomaly = self.solve_eccentric_anomaly(mean_anomaly);

        // True anomaly
        let true_anomaly = self.eccentric_to_true_anomaly(e_anomaly);

        // Orbital parameters
        let e = self.eccentricity;
        let a = self.semi_major_axis;
        let n = self.mean_motion;

        // Vis-viva: velocity components in orbital frame
        // Using: h = n * a^2 * sqrt(1 - e^2) (specific angular momentum)
        let h = n * a * a * (1.0 - e * e).sqrt();
        let r = self.radius(e_anomaly);

        // Velocity in perifocal frame
        let vr = h * e * true_anomaly.sin() / (a * (1.0 - e * e)); // radial
        let vt = h / r; // tangential

        // Rotate to ecliptic frame
        let angle = true_anomaly + self.argument_of_periapsis;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        DVec2::new(
            vr * cos_a - vt * sin_a,
            vr * sin_a + vt * cos_a,
        )
    }

    /// Orbital period in seconds.
    pub fn period(&self) -> f64 {
        std::f64::consts::TAU / self.mean_motion
    }

    /// Orbital period in days.
    pub fn period_days(&self) -> f64 {
        self.period() / SECONDS_PER_DAY
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AU_TO_METERS;

    /// Earth's approximate orbital elements
    fn earth_orbit() -> KeplerOrbit {
        KeplerOrbit::from_elements(
            1.0 * AU_TO_METERS,  // 1 AU
            0.0167,              // eccentricity
            102.94,              // argument of periapsis (degrees)
            357.53,              // mean anomaly at epoch (degrees)
            0.9856,              // mean motion (degrees/day)
        )
    }

    #[test]
    fn test_kepler_solver_circular() {
        // Test with circular orbit (e=0)
        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            0.0,      // circular
            0.0,
            0.0,
            0.9856,
        );

        // For circular orbit, E = M
        let m = 1.0; // radians
        let e = orbit.solve_eccentric_anomaly(m);
        assert!((e - m).abs() < 1e-10, "Circular orbit: E should equal M");
    }

    #[test]
    fn test_kepler_solver_elliptical() {
        // Test with Mercury-like eccentricity
        let orbit = KeplerOrbit::from_elements(
            0.387 * AU_TO_METERS,
            0.2056,   // Mercury's eccentricity
            29.12,
            174.79,
            4.0923,
        );

        // Verify Kepler's equation: M = E - e*sin(E)
        let m = 1.5; // radians
        let e_anom = orbit.solve_eccentric_anomaly(m);
        let m_check = e_anom - orbit.eccentricity * e_anom.sin();
        let m_normalized = m.rem_euclid(std::f64::consts::TAU);
        assert!(
            (m_check - m_normalized).abs() < 1e-10,
            "Kepler equation not satisfied: {} vs {}",
            m_check,
            m_normalized
        );
    }

    #[test]
    fn test_kepler_solver_high_eccentricity() {
        // Test with high eccentricity (e=0.9)
        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            0.9,
            0.0,
            0.0,
            0.1,
        );

        // Verify Kepler's equation
        for m in [0.1, 0.5, 1.0, 2.0, 3.0, 5.0] {
            let e_anom = orbit.solve_eccentric_anomaly(m);
            let m_check = e_anom - orbit.eccentricity * e_anom.sin();
            let m_normalized = m.rem_euclid(std::f64::consts::TAU);
            assert!(
                (m_check - m_normalized).abs() < 1e-10,
                "High eccentricity: Kepler equation not satisfied for M={}: {} vs {}",
                m, m_check, m_normalized
            );
        }
    }

    #[test]
    fn test_earth_position_at_epoch() {
        let orbit = earth_orbit();

        // At J2000 epoch (time = 0)
        let pos = orbit.get_local_position(0.0);

        // Earth should be roughly 1 AU from Sun
        let distance = pos.length();
        let distance_au = distance / AU_TO_METERS;

        assert!(
            (distance_au - 1.0).abs() < 0.02,
            "Earth should be ~1 AU from Sun, got {} AU",
            distance_au
        );
    }

    #[test]
    fn test_earth_orbital_period() {
        let orbit = earth_orbit();
        let period_days = orbit.period_days();

        // Earth's orbital period should be ~365.25 days
        assert!(
            (period_days - 365.25).abs() < 1.0,
            "Earth orbital period should be ~365.25 days, got {} days",
            period_days
        );
    }

    #[test]
    fn test_position_periodicity() {
        let orbit = earth_orbit();
        let period = orbit.period();

        // Position should be the same after one complete orbit
        let pos1 = orbit.get_local_position(0.0);
        let pos2 = orbit.get_local_position(period);

        let diff = (pos2 - pos1).length();
        assert!(
            diff < 1000.0, // within 1 km after full orbit
            "Position should repeat after one period, diff = {} m",
            diff
        );
    }

    #[test]
    fn test_velocity_magnitude() {
        let orbit = earth_orbit();
        let vel = orbit.get_local_velocity(0.0);
        let speed_km_s = vel.length() / 1000.0;

        // Earth's orbital velocity is ~29.78 km/s
        assert!(
            (speed_km_s - 29.78).abs() < 1.0,
            "Earth orbital velocity should be ~29.78 km/s, got {} km/s",
            speed_km_s
        );
    }

    #[test]
    fn test_true_anomaly_full_orbit() {
        // Test that true anomaly covers full range using atan2
        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            0.5, // moderate eccentricity
            0.0,
            0.0,
            0.9856,
        );

        // Test at various eccentric anomalies including near π
        for e_deg in [0.0, 45.0, 90.0, 135.0, 179.0, 180.0, 181.0, 270.0, 359.0] {
            let e_rad = e_deg * std::f64::consts::PI / 180.0;
            let nu = orbit.eccentric_to_true_anomaly(e_rad);

            // True anomaly should be in valid range
            assert!(
                nu.is_finite(),
                "True anomaly should be finite for E = {} deg",
                e_deg
            );
        }
    }

    // ===== ROBUSTNESS TESTS (currently ignored, enable when solver is improved) =====

    #[test]
    #[ignore = "Near-parabolic orbits need improved solver (Halley's method or bisection fallback)"]
    fn test_kepler_solver_near_parabolic() {
        // Test with e=0.99 (comet-like orbit)
        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            0.99,
            0.0,
            0.0,
            0.01,
        );

        // Near perihelion (M close to 0), Newton's method may struggle
        for m in [0.001, 0.01, 0.1, 3.14, 6.28] {
            let e_anom = orbit.solve_eccentric_anomaly(m);
            let m_check = e_anom - orbit.eccentricity * e_anom.sin();
            let m_normalized = m.rem_euclid(std::f64::consts::TAU);
            assert!(
                (m_check - m_normalized).abs() < 1e-10,
                "Near-parabolic (e=0.99): Kepler equation not satisfied for M={}: {} vs {}",
                m, m_check, m_normalized
            );
        }
    }

    #[test]
    #[ignore = "Hyperbolic orbits not yet supported (need different equation)"]
    fn test_kepler_solver_hyperbolic() {
        // Hyperbolic orbit (e > 1) uses different equation:
        // M = e*sinh(H) - H (where H is hyperbolic anomaly)
        // This test is a placeholder for future implementation
        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            1.5, // hyperbolic
            0.0,
            0.0,
            0.01,
        );

        // Should handle gracefully or return error
        let _ = orbit.solve_eccentric_anomaly(1.0);
    }

    #[test]
    #[ignore = "Convergence detection not yet implemented"]
    fn test_kepler_solver_convergence_detection() {
        // Test that solver detects non-convergence and handles it
        // Currently just silently returns after 50 iterations
        let orbit = KeplerOrbit::from_elements(
            AU_TO_METERS,
            0.999, // extremely high eccentricity
            0.0,
            0.0,
            0.01,
        );

        // This may not converge properly - solver should detect and handle
        let e_anom = orbit.solve_eccentric_anomaly(0.001);
        let m_check = e_anom - orbit.eccentricity * e_anom.sin();

        // When convergence detection is implemented, this should either:
        // - Return a Result::Err, or
        // - Fall back to bisection and succeed
        assert!(
            (m_check - 0.001).abs() < 1e-8,
            "Solver should converge or report failure"
        );
    }
}
