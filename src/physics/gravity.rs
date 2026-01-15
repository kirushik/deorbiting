//! Gravity calculation for asteroid physics.
//!
//! Computes gravitational acceleration from all celestial bodies
//! using the ephemeris as the source of truth for positions.

use bevy::math::DVec2;

use crate::ephemeris::Ephemeris;
use crate::types::GM_SUN;

use crate::ephemeris::GravitySources;

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
#[inline]
pub fn compute_acceleration(pos: DVec2, time: f64, ephemeris: &Ephemeris) -> DVec2 {
    compute_acceleration_from_sources(pos, &ephemeris.get_gravity_sources(time))
}

/// Compute gravitational acceleration from pre-fetched gravity sources.
///
/// This is more efficient when multiple calculations need the same sources
/// (e.g., in trajectory prediction loops where positions are sampled at the same time).
///
/// # Arguments
/// * `pos` - Position in meters from solar system barycenter
/// * `sources` - Pre-fetched array of (position, GM) pairs
///
/// # Returns
/// Acceleration vector in m/s²
#[inline]
pub fn compute_acceleration_from_sources(pos: DVec2, sources: &GravitySources) -> DVec2 {
    let mut acc = DVec2::ZERO;

    for &(body_pos, gm) in sources {
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


/// Information about the closest celestial body for timestep adaptation.
#[derive(Debug, Clone, Copy)]
pub struct ClosestBodyInfo {
    /// Distance to the closest body (meters)
    pub distance: f64,
    /// Velocity of the closest body (m/s)
    pub body_velocity: DVec2,
    /// Collision radius of the closest body (with multiplier applied)
    pub collision_radius: f64,
}

/// Compute the closest celestial body to a given position.
///
/// Returns information about the closest body, which is used for
/// timestep adaptation to ensure we don't "skip over" bodies.
pub fn find_closest_body(pos: DVec2, time: f64, ephemeris: &Ephemeris) -> Option<ClosestBodyInfo> {
    use crate::ephemeris::{CelestialBodyId, COLLISION_MULTIPLIER};

    let mut closest: Option<ClosestBodyInfo> = None;
    let mut min_distance = f64::MAX;

    // Check all planets
    for &id in CelestialBodyId::PLANETS {
        if let (Some(body_pos), Some(data)) = (
            ephemeris.get_position_by_id(id, time),
            ephemeris.get_body_data_by_id(id),
        ) {
            let distance = (pos - body_pos).length();
            if distance < min_distance {
                min_distance = distance;
                // Approximate body velocity from orbital mechanics (circular orbit assumption)
                // v = sqrt(GM_sun / r)
                let r = body_pos.length();
                let v_mag = (GM_SUN / r).sqrt();
                let angle = body_pos.y.atan2(body_pos.x);
                let body_velocity = DVec2::new(-angle.sin(), angle.cos()) * v_mag;

                closest = Some(ClosestBodyInfo {
                    distance,
                    body_velocity,
                    collision_radius: data.radius * COLLISION_MULTIPLIER,
                });
            }
        }
    }

    // Also check Sun
    if let Some(data) = ephemeris.get_body_data_by_id(CelestialBodyId::Sun) {
        let distance = pos.length(); // Sun at origin
        if distance < min_distance {
            closest = Some(ClosestBodyInfo {
                distance,
                body_velocity: DVec2::ZERO, // Sun doesn't move
                collision_radius: data.radius * 2.0, // Sun uses smaller multiplier
            });
        }
    }

    closest
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AU_TO_METERS, GM_SUN};

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
        let expected_mag = GM_SUN / (AU_TO_METERS * AU_TO_METERS);
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

    #[test]
    fn test_planet_gravity_contributes() {
        use crate::ephemeris::CelestialBodyId;

        let ephemeris = Ephemeris::default();

        // Get Jupiter's position at J2000
        let jupiter_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Jupiter, 0.0)
            .expect("Jupiter should have position");

        // Place asteroid 0.05 AU from Jupiter (within Hill sphere)
        let offset = DVec2::new(0.05 * AU_TO_METERS, 0.0);
        let asteroid_pos = jupiter_pos + offset;

        // Compute acceleration
        let acc = compute_acceleration(asteroid_pos, 0.0, &ephemeris);

        // The acceleration should have a significant component toward Jupiter (negative x relative to offset)
        // Let's compute what we'd expect from Sun alone vs Sun+Jupiter

        // Sun-only acceleration (approximately)
        let sun_dist = asteroid_pos.length();
        let sun_acc_mag = GM_SUN / (sun_dist * sun_dist);

        // Jupiter acceleration
        let jupiter_gm = 6.67430e-11 * 1.898e27; // G * M_jupiter
        let jupiter_dist = offset.length();
        let jupiter_acc_mag = jupiter_gm / (jupiter_dist * jupiter_dist);

        // At 0.05 AU from Jupiter, Jupiter's gravity should be significant
        // Jupiter GM / (0.05 AU)² = 1.27e17 / (7.5e9)² ≈ 2.3e-3 m/s²
        // Sun at ~5 AU: 1.33e20 / (7.8e11)² ≈ 2.2e-4 m/s²
        // So Jupiter should dominate by ~10x at this distance

        let ratio = jupiter_acc_mag / sun_acc_mag;
        assert!(
            ratio > 5.0,
            "Jupiter should dominate at 0.05 AU distance. Ratio: {:.2}",
            ratio
        );

        // Print for debugging
        println!("Jupiter distance: {:.4e} m ({:.4} AU)", jupiter_dist, jupiter_dist / AU_TO_METERS);
        println!("Sun distance: {:.4e} m ({:.4} AU)", sun_dist, sun_dist / AU_TO_METERS);
        println!("Jupiter acc magnitude: {:.4e} m/s²", jupiter_acc_mag);
        println!("Sun acc magnitude: {:.4e} m/s²", sun_acc_mag);
        println!("Jupiter/Sun ratio: {:.2}", ratio);
        println!("Total acceleration: ({:.4e}, {:.4e}) m/s²", acc.x, acc.y);

        // Verify total acceleration magnitude is higher than Sun alone
        // (because Jupiter adds to it significantly)
        let total_mag = acc.length();
        assert!(
            total_mag > sun_acc_mag * 1.5,
            "Total acceleration should be much higher than Sun alone"
        );
    }

    #[test]
    fn test_gravity_sources_count() {
        let ephemeris = Ephemeris::default();
        let sources = ephemeris.get_gravity_sources(0.0);

        // Should have Sun + 8 planets + moons
        // At minimum: Sun, Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune = 9
        assert!(
            sources.len() >= 9,
            "Should have at least 9 gravity sources (Sun + 8 planets), got {}",
            sources.len()
        );

        // Print all sources for debugging
        println!("Gravity sources ({} total):", sources.len());
        for (i, (pos, gm)) in sources.iter().enumerate() {
            println!(
                "  {}: pos=({:.3e}, {:.3e}), GM={:.4e}",
                i, pos.x, pos.y, gm
            );
        }
    }
}
