//! Rendering systems for the orbital mechanics simulator.
//!
//! This module provides visual representation of celestial bodies,
//! trajectory lines, and background elements.

mod background;
mod bodies;
mod highlight;
mod labels;
mod orbits;
mod sync;

use bevy::prelude::*;

use self::background::BackgroundPlugin;
use self::bodies::CelestialBodyPlugin;
use self::highlight::HighlightPlugin;
use self::labels::LabelPlugin;
use self::orbits::OrbitPathPlugin;
use self::sync::sync_celestial_positions;

// Re-export for use in other modules
#[allow(unused_imports)]
pub use self::bodies::CelestialBody;

/// Plugin aggregating all rendering functionality.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CelestialBodyPlugin,
            BackgroundPlugin,
            OrbitPathPlugin,
            HighlightPlugin,
            LabelPlugin,
        ))
        .add_systems(Update, sync_celestial_positions);
    }
}

/// Z-layer constants for rendering order.
pub mod z_layers {
    /// Background elements (starfield).
    pub const BACKGROUND: f32 = 0.0;
    /// Trajectory prediction lines.
    pub const TRAJECTORY: f32 = 1.0;
    /// Celestial bodies (Sun, planets, moons).
    pub const CELESTIAL: f32 = 2.0;
    /// Spacecraft and asteroids.
    pub const SPACECRAFT: f32 = 3.0;
    /// UI handles (velocity arrows, etc.).
    pub const UI_HANDLES: f32 = 4.0;
}
