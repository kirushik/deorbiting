//! Gravity calculation for asteroid physics.
//!
//! Computes gravitational acceleration from all celestial bodies
//! using the ephemeris as the source of truth for positions.

use bevy::math::DVec2;

use crate::ephemeris::Ephemeris;

/// Compute gravitational acceleration at a given position and time.
///
/// Uses all gravity sources from the ephemeris (Sun, planets, moons).
/// The ephemeris returns GM values (G already multiplied), so no G multiplication needed.
///
/// # Arguments
/// * `pos` - Position in meters from solar system barycenter
/// * `time` - Simulation time in seconds since J2000
/// * `ephemeris` - Reference to ephemeris resource
///
/// # Returns
/// Acceleration vector in m/s²
pub fn compute_acceleration(pos: DVec2, time: f64, ephemeris: &Ephemeris) -> DVec2 {
    let mut acc = DVec2::ZERO;

    // ephemeris.get_gravity_sources returns (position, GM) pairs
    // where GM is already G * mass (standard gravitational parameter)
    for (body_pos, gm) in ephemeris.get_gravity_sources(time) {
        let delta = body_pos - pos;
        let r_squared = delta.length_squared();

        // Avoid singularity at very small distances.
        // 1.0 meter threshold is safe - no real orbits that close.
        if r_squared > 1.0 {
            let r = r_squared.sqrt();
            // a = GM/r² in the direction of delta (toward the body)
            // delta/r gives the unit vector
            acc += delta * (gm / (r_squared * r));
        }
    }

    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AU_TO_METERS;

    #[test]
    fn test_acceleration_at_earth_distance() {
        let ephemeris = Ephemeris::default();

        // Position at 1 AU from Sun along x-axis
        let pos = DVec2::new(AU_TO_METERS, 0.0);

        // Compute acceleration at J2000 epoch
        let acc = compute_acceleration(pos, 0.0, &ephemeris);

        // Acceleration should be roughly toward the Sun (negative x)
        assert!(acc.x < 0.0, "Acceleration should be toward Sun");

        // Expected magnitude: GM_sun / r² ≈ 5.93e-3 m/s²
        let expected_mag = 1.32712440018e20 / (AU_TO_METERS * AU_TO_METERS);
        let actual_mag = acc.length();

        // Allow 1% error due to other bodies' influence
        let error = (actual_mag - expected_mag).abs() / expected_mag;
        assert!(
            error < 0.01,
            "Acceleration magnitude error: {:.2}% (got {:.6e}, expected {:.6e})",
            error * 100.0,
            actual_mag,
            expected_mag
        );
    }

    #[test]
    fn test_acceleration_near_singularity() {
        let ephemeris = Ephemeris::default();

        // Position very close to Sun (inside singularity threshold)
        let pos = DVec2::new(0.5, 0.0);
        let acc = compute_acceleration(pos, 0.0, &ephemeris);

        // Should not produce NaN or infinity
        assert!(acc.x.is_finite(), "Acceleration should be finite");
        assert!(acc.y.is_finite(), "Acceleration should be finite");
    }
}
