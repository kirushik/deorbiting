//! Visual distortion to handle visually-inflated celestial bodies.
//!
//! When planets are rendered larger than their physical size for visibility,
//! nearby objects (like asteroids) would appear to clip through the visual
//! representation. This module provides functions to distort visual positions
//! to maintain correct apparent distances.

use bevy::math::DVec2;

use crate::ephemeris::{CelestialBodyId, Ephemeris};

/// Apply visual distortion to push an object away from a visually-inflated planet.
///
/// When a planet is rendered larger than its physical radius, objects near
/// the surface would appear to be inside the planet. This function adjusts
/// the visual position to maintain the correct apparent distance from the
/// planet's visual surface.
///
/// # Arguments
/// * `obj_pos` - Object's physics position in meters
/// * `planet_pos` - Planet's physics position in meters
/// * `phys_r` - Planet's physical radius in meters
/// * `visual_scale` - Planet's visual scale factor (render radius = phys_r * visual_scale)
///
/// # Returns
/// The distorted position for rendering
pub fn apply_visual_distortion(
    obj_pos: DVec2,
    planet_pos: DVec2,
    phys_r: f64,
    visual_scale: f32,
) -> DVec2 {
    let delta = obj_pos - planet_pos;
    let dist = delta.length();

    // Object is at planet center - no meaningful distortion
    if dist < 1e-10 {
        return obj_pos;
    }

    let visual_r = phys_r * (visual_scale as f64);
    let radius_delta = visual_r - phys_r;

    // Push the object outward by the difference between visual and physical radius
    let new_dist = dist + radius_delta;
    let dir = delta.normalize();

    planet_pos + (dir * new_dist)
}

/// Find the nearest planet to a given position.
///
/// Only considers planets (not the Sun or moons) for distortion purposes.
///
/// # Arguments
/// * `pos` - Position to find nearest planet to (in meters)
/// * `ephemeris` - Ephemeris resource for looking up positions
/// * `time` - Current simulation time (seconds since J2000)
///
/// # Returns
/// Tuple of (planet_id, planet_position, physical_radius, visual_scale) for the nearest planet,
/// or None if no planets are registered.
pub fn find_nearest_planet(
    pos: DVec2,
    ephemeris: &Ephemeris,
    time: f64,
) -> Option<(CelestialBodyId, DVec2, f64, f32)> {
    let mut nearest: Option<(CelestialBodyId, DVec2, f64, f32, f64)> = None;

    for &planet_id in CelestialBodyId::PLANETS {
        let Some(planet_pos) = ephemeris.get_position_by_id(planet_id, time) else {
            continue;
        };

        if let Some(data) = ephemeris.get_body_data_by_id(planet_id) {
            let dist = (pos - planet_pos).length();

            let should_update = nearest.is_none_or(|(_, _, _, _, d)| dist < d);

            if should_update {
                nearest = Some((planet_id, planet_pos, data.radius, data.visual_scale, dist));
            }
        }
    }

    nearest.map(|(id, pos, r, vs, _)| (id, pos, r, vs))
}

/// Apply visual distortion relative to the nearest planet.
///
/// Convenience function that combines `find_nearest_planet` and `apply_visual_distortion`.
///
/// # Arguments
/// * `obj_pos` - Object's physics position in meters
/// * `ephemeris` - Ephemeris resource
/// * `time` - Current simulation time
///
/// # Returns
/// The distorted position for rendering, or the original position if no planet is nearby.
pub fn distort_position(obj_pos: DVec2, ephemeris: &Ephemeris, time: f64) -> DVec2 {
    if let Some((_, planet_pos, phys_r, visual_scale)) = find_nearest_planet(obj_pos, ephemeris, time) {
        apply_visual_distortion(obj_pos, planet_pos, phys_r, visual_scale)
    } else {
        obj_pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distortion_pushes_outward() {
        let planet_pos = DVec2::ZERO;
        let obj_pos = DVec2::new(1000.0, 0.0); // 1km from center
        let phys_r = 100.0; // 100m physical radius
        let visual_scale = 10.0; // 10x visual scale

        let distorted = apply_visual_distortion(obj_pos, planet_pos, phys_r, visual_scale);

        // Visual radius = 1000m, physical radius = 100m
        // Radius delta = 900m
        // Object is at 1000m, so distorted = 1000 + 900 = 1900m
        assert!(
            (distorted.x - 1900.0).abs() < 0.01,
            "Expected x ≈ 1900, got {}",
            distorted.x
        );
        assert!(distorted.y.abs() < 0.01, "Expected y ≈ 0, got {}", distorted.y);
    }

    #[test]
    fn test_distortion_direction_preserved() {
        let planet_pos = DVec2::new(100.0, 100.0);
        let obj_pos = DVec2::new(100.0, 200.0); // Directly above planet
        let phys_r = 10.0;
        let visual_scale = 5.0;

        let distorted = apply_visual_distortion(obj_pos, planet_pos, phys_r, visual_scale);

        // Direction should be preserved (still directly above)
        assert!(
            (distorted.x - 100.0).abs() < 0.01,
            "Expected x = 100, got {}",
            distorted.x
        );
        // Should be pushed further away
        assert!(
            distorted.y > obj_pos.y,
            "Expected y > {}, got {}",
            obj_pos.y,
            distorted.y
        );
    }

    #[test]
    fn test_distortion_at_center() {
        let planet_pos = DVec2::new(50.0, 50.0);
        let obj_pos = planet_pos; // Object at planet center
        let phys_r = 100.0;
        let visual_scale = 5.0;

        let distorted = apply_visual_distortion(obj_pos, planet_pos, phys_r, visual_scale);

        // Should return original position (can't determine direction)
        assert_eq!(distorted, obj_pos);
    }

    #[test]
    fn test_distortion_with_no_scale() {
        let planet_pos = DVec2::ZERO;
        let obj_pos = DVec2::new(500.0, 0.0);
        let phys_r = 100.0;
        let visual_scale = 1.0; // No visual inflation

        let distorted = apply_visual_distortion(obj_pos, planet_pos, phys_r, visual_scale);

        // No distortion when visual_scale = 1.0
        assert!(
            (distorted.x - 500.0).abs() < 0.01,
            "Expected x = 500, got {}",
            distorted.x
        );
    }

    #[test]
    fn test_distortion_diagonal() {
        let planet_pos = DVec2::ZERO;
        let obj_pos = DVec2::new(100.0, 100.0); // Diagonal from planet
        let phys_r = 10.0;
        let visual_scale = 2.0; // Double visual size

        let distorted = apply_visual_distortion(obj_pos, planet_pos, phys_r, visual_scale);

        // Original distance ≈ 141.42
        // Radius delta = 10
        // New distance ≈ 151.42
        let orig_dist = obj_pos.length();
        let new_dist = distorted.length();

        assert!(
            (new_dist - orig_dist - 10.0).abs() < 0.01,
            "Expected distance increase of 10, got {}",
            new_dist - orig_dist
        );

        // Direction should be preserved
        let orig_dir = obj_pos.normalize();
        let new_dir = distorted.normalize();
        assert!(
            (orig_dir - new_dir).length() < 0.001,
            "Direction not preserved"
        );
    }
}
