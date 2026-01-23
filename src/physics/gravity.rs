//! Gravity calculation for asteroid physics.
//!
//! Computes gravitational acceleration from all celestial bodies
//! using the ephemeris as the source of truth for positions.

use bevy::math::DVec2;

use crate::ephemeris::Ephemeris;
use crate::types::GM_SUN;

use crate::ephemeris::{CelestialBodyId, GravitySources, GravitySourcesFull};

/// Minimum squared distance threshold for gravity calculations (meters²).
///
/// Below this threshold, gravitational acceleration is clamped to avoid
/// numerical singularities. 1e6 m² = 1 km² is a safe threshold that
/// prevents NaN/Inf while allowing realistic close approaches.
const SINGULARITY_THRESHOLD_SQ: f64 = 1e6;

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

        // Avoid singularity at very small distances
        if r_squared > SINGULARITY_THRESHOLD_SQ {
            let r = r_squared.sqrt();
            // a = GM/r² in the direction of delta (toward the body)
            // delta/r gives the unit vector
            acc += delta * (gm / (r_squared * r));
        }
    }

    acc
}

/// Result of combined gravity, dominant body, and collision computation.
///
/// This struct holds all the physics information computed from a single
/// ephemeris lookup, avoiding redundant position queries.
#[derive(Debug, Clone, Copy)]
pub struct GravityResult {
    /// Gravitational acceleration vector in m/s²
    pub acceleration: DVec2,
    /// The celestial body whose gravity dominates at this position.
    /// None if the Sun dominates (the default case).
    pub dominant_body: Option<CelestialBodyId>,
    /// If a collision was detected, the body that was hit.
    pub collision: Option<CelestialBodyId>,
}

/// Compute acceleration, dominant body, and collision check in a SINGLE pass.
///
/// This is significantly more efficient than calling separate functions,
/// as it avoids redundant ephemeris lookups. For trajectory prediction,
/// this reduces ephemeris queries from 24 per step to 8.
///
/// # Arguments
/// * `pos` - Position in meters from solar system barycenter
/// * `sources` - Pre-fetched array of full gravity source data
///
/// # Returns
/// A `GravityResult` containing acceleration, dominant body, and collision info.
#[inline]
pub fn compute_gravity_full(pos: DVec2, sources: &GravitySourcesFull) -> GravityResult {
    let mut acc = DVec2::ZERO;
    let mut max_acc_mag = 0.0_f64;
    let mut dominant = CelestialBodyId::Sun;
    let mut collision = None;

    for source in sources {
        let delta = source.pos - pos;
        let r_squared = delta.length_squared();

        // Check collision first (before singularity guard)
        let dist = r_squared.sqrt();
        if dist < source.collision_radius && collision.is_none() {
            collision = Some(source.id);
        }

        // Gravity computation with singularity guard
        // MUST use same threshold as compute_acceleration_from_sources() for consistency
        if r_squared > SINGULARITY_THRESHOLD_SQ {
            let factor = source.gm / (r_squared * dist);
            acc += delta * factor;

            // Track dominant body (highest acceleration magnitude)
            let acc_mag = source.gm / r_squared;
            if acc_mag > max_acc_mag {
                max_acc_mag = acc_mag;
                dominant = source.id;
            }
        }
    }

    GravityResult {
        acceleration: acc,
        dominant_body: if dominant == CelestialBodyId::Sun {
            None
        } else {
            Some(dominant)
        },
        collision,
    }
}

/// Compute acceleration from full gravity sources (ignoring dominant body/collision).
///
/// Use this when you only need acceleration but have already fetched full sources.
#[inline]
pub fn compute_acceleration_from_full_sources(pos: DVec2, sources: &GravitySourcesFull) -> DVec2 {
    let mut acc = DVec2::ZERO;

    for source in sources {
        let delta = source.pos - pos;
        let r_squared = delta.length_squared();

        // MUST use same threshold as compute_acceleration_from_sources() for consistency
        if r_squared > SINGULARITY_THRESHOLD_SQ {
            let r = r_squared.sqrt();
            acc += delta * (source.gm / (r_squared * r));
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
    use crate::ephemeris::{COLLISION_MULTIPLIER, CelestialBodyId};

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
                body_velocity: DVec2::ZERO,          // Sun doesn't move
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
        println!(
            "Jupiter distance: {:.4e} m ({:.4} AU)",
            jupiter_dist,
            jupiter_dist / AU_TO_METERS
        );
        println!(
            "Sun distance: {:.4e} m ({:.4} AU)",
            sun_dist,
            sun_dist / AU_TO_METERS
        );
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

        // Should have Sun + 8 planets = 9 gravity sources (moons are decorative only)
        assert_eq!(
            sources.len(),
            9,
            "Should have 9 gravity sources (Sun + 8 planets), got {}",
            sources.len()
        );

        // Print all sources for debugging
        println!("Gravity sources ({} total):", sources.len());
        for (i, (pos, gm)) in sources.iter().enumerate() {
            println!("  {}: pos=({:.3e}, {:.3e}), GM={:.4e}", i, pos.x, pos.y, gm);
        }
    }

    /// Test that prediction gravity matches physics gravity.
    ///
    /// BUG: `compute_gravity_full()` (used in trajectory prediction) and
    /// `compute_acceleration_from_sources()` (used in main physics) use
    /// DIFFERENT singularity thresholds:
    /// - prediction: 1.0 m² threshold (includes objects at 100m)
    /// - physics: 1e6 m² threshold (excludes objects closer than 1000m)
    ///
    /// This causes trajectory prediction to diverge from actual physics
    /// when asteroids pass within 1000m of a body but outside 1m.
    #[test]
    fn test_prediction_gravity_matches_physics_gravity() {
        // Use ephemeris to get properly-formatted gravity sources
        let ephemeris = Ephemeris::default();

        // Get gravity sources at J2000
        let simple_sources = ephemeris.get_gravity_sources(0.0);
        let full_sources = ephemeris.get_gravity_sources_full(0.0);

        // Test at various distances from the Sun (at origin)
        // The key range is between 1m (prediction threshold) and 1000m (physics threshold)
        let test_distances = [
            0.5,    // 0.5m - both should clamp (below both thresholds)
            10.0,   // 10m - prediction includes, physics excludes
            100.0,  // 100m - prediction includes, physics excludes
            500.0,  // 500m - prediction includes, physics excludes
            1000.0, // 1000m = exactly at physics threshold
            2000.0, // 2000m - both should include (above both thresholds)
            1e6,    // 1 million m - both should include
        ];

        println!("\nGravity consistency test (near Sun):");
        println!("  Distance |   Physics acc   | Prediction acc  |   Difference   | Status");
        println!("  ---------|-----------------|-----------------|----------------|-------");

        let mut found_mismatch = false;

        for &dist in &test_distances {
            let pos = DVec2::new(dist, 0.0);

            // Physics calculation (used by main simulation)
            let physics_acc = compute_acceleration_from_sources(pos, &simple_sources);

            // Prediction calculation (used by trajectory prediction)
            let prediction_result = compute_gravity_full(pos, &full_sources);
            let prediction_acc = prediction_result.acceleration;

            let physics_mag = physics_acc.length();
            let prediction_mag = prediction_acc.length();

            // Check if they match (either both zero or both equal)
            let both_zero = physics_mag == 0.0 && prediction_mag == 0.0;
            let magnitudes_match = if physics_mag > 0.0 && prediction_mag > 0.0 {
                ((prediction_mag - physics_mag) / physics_mag).abs() < 0.01 // 1% tolerance
            } else {
                both_zero
            };

            // Key check: if one is zero and other isn't, that's a mismatch
            let one_zero_other_not = (physics_mag == 0.0) != (prediction_mag == 0.0);

            let status = if one_zero_other_not || !magnitudes_match {
                found_mismatch = true;
                "MISMATCH"
            } else {
                "ok"
            };

            let diff_str = if one_zero_other_not {
                if physics_mag == 0.0 {
                    format!("physics=0, pred≠0")
                } else {
                    format!("physics≠0, pred=0")
                }
            } else if both_zero {
                format!("both zero")
            } else {
                format!(
                    "{:.1}%",
                    ((prediction_mag - physics_mag) / physics_mag).abs() * 100.0
                )
            };

            println!(
                "  {:>8.1}m | {:>13.4e} | {:>13.4e} | {:>14} | {}",
                dist, physics_mag, prediction_mag, diff_str, status
            );
        }

        // THE CORE ASSERTION: prediction gravity MUST match physics gravity
        // This test SHOULD FAIL until we fix the singularity threshold inconsistency
        assert!(
            !found_mismatch,
            "\n\nBUG DETECTED: Trajectory prediction uses different gravity than physics!\n\
             \n\
             Root cause: compute_gravity_full() uses singularity threshold of 1.0 m²,\n\
             while compute_acceleration_from_sources() uses 1e6 m² (SINGULARITY_THRESHOLD_SQ).\n\
             \n\
             Impact: Predicted trajectories diverge from actual simulation when\n\
             asteroids pass within 1000m of a body.\n\
             \n\
             Fix: Make compute_gravity_full() and compute_acceleration_from_full_sources()\n\
             use SINGULARITY_THRESHOLD_SQ instead of hardcoded 1.0."
        );
    }
}
