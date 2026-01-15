//! Position synchronization between physics and rendering.
//!
//! Updates visual Transform positions from physics state (BodyState for asteroids)
//! or Ephemeris resource (for celestial bodies).

use bevy::{math::DVec2, prelude::*};

use crate::asteroid::{Asteroid, AsteroidVisual};
use crate::camera::{CameraState, RENDER_SCALE};
use crate::distortion::distort_position;
use crate::ephemeris::Ephemeris;
use crate::render::bodies::CelestialBody;
use crate::render::z_layers;
use crate::types::{BodyState, SimulationTime};

/// System set label for position sync (runs before scaling/distortion).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SyncPositionsSet;

/// Sync celestial body render positions from ephemeris data.
///
/// This system reads physics positions from the Ephemeris resource and
/// updates the Transform components for rendering.
pub fn sync_celestial_positions(
    mut query: Query<(Entity, &mut Transform, &CelestialBody)>,
    ephemeris: Res<Ephemeris>,
    time: Res<SimulationTime>,
) {
    for (entity, mut transform, _body) in query.iter_mut() {
        // Get physics position from ephemeris
        let pos = ephemeris
            .get_position(entity, time.current)
            .unwrap_or(DVec2::ZERO);

        // Convert f64 meters to f32 render units
        transform.translation.x = (pos.x * RENDER_SCALE) as f32;
        transform.translation.y = (pos.y * RENDER_SCALE) as f32;
        transform.translation.z = z_layers::CELESTIAL;
    }
}

/// Sync asteroid render positions from BodyState physics data.
///
/// Unlike celestial bodies which read from ephemeris, asteroids have their
/// positions computed by the physics integrator and stored in BodyState.
/// This system also applies visual distortion to prevent clipping with
/// inflated planets, and scales asteroids to maintain constant screen-space size.
pub fn sync_asteroid_positions(
    mut query: Query<(&mut Transform, &BodyState, &AsteroidVisual), With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    time: Res<SimulationTime>,
    camera: Res<CameraState>,
) {
    // Target screen-space size: asteroids appear as ~0.8% of viewport height
    // This keeps them visible as markers without dominating the view
    const TARGET_SCREEN_FRACTION: f32 = 0.008;
    const VIEWPORT_HEIGHT: f32 = 20.0; // Same as in scaling.rs
    
    let viewport_size = VIEWPORT_HEIGHT * camera.zoom;
    let target_size = viewport_size * TARGET_SCREEN_FRACTION;
    
    for (mut transform, body_state, visual) in query.iter_mut() {
        // Apply visual distortion relative to nearest planet
        // This prevents the asteroid from appearing to clip through
        // visually-inflated planets
        let distorted_pos = distort_position(body_state.pos, &ephemeris, time.current);

        // Convert f64 meters to f32 render units
        transform.translation.x = (distorted_pos.x * RENDER_SCALE) as f32;
        transform.translation.y = (distorted_pos.y * RENDER_SCALE) as f32;
        transform.translation.z = z_layers::SPACECRAFT;
        
        // Scale asteroid to maintain constant screen-space size
        // This makes them work like UI markers rather than physical objects
        let base_size = visual.render_radius;
        let scale = (target_size / base_size).clamp(0.1, 10.0);
        transform.scale = Vec3::splat(scale);
    }
}
