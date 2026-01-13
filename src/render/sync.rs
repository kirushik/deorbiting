//! Position synchronization between physics and rendering.
//!
//! Updates visual Transform positions from the Ephemeris resource.

use bevy::{math::DVec2, prelude::*};

use crate::camera::RENDER_SCALE;
use crate::ephemeris::Ephemeris;
use crate::render::bodies::CelestialBody;
use crate::render::z_layers;
use crate::types::SimulationTime;

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
