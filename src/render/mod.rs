//! Rendering systems for the orbital mechanics simulator.
//!
//! This module provides visual representation of celestial bodies,
//! trajectory lines, and background elements.

mod background;
pub mod bodies;
pub mod highlight;
mod labels;
mod orbits;
pub mod scaling;
mod sync;

use bevy::prelude::*;

use self::background::BackgroundPlugin;
use self::bodies::CelestialBodyPlugin;
use self::highlight::HighlightPlugin;
use self::labels::LabelPlugin;
use self::orbits::{draw_moon_orbit_paths, draw_orbit_paths, OrbitPathPlugin};
use self::scaling::{
    apply_moon_position_distortion, compute_hierarchical_scales, ScalingPlugin,
};
use self::sync::sync_celestial_positions;

// Re-export for use in other modules
pub use self::bodies::{CelestialBody, DistortionOffset, EffectiveVisualRadius};
pub use self::highlight::{HoveredBody, SelectedBody};
pub use self::scaling::ScalingSettings;

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
            ScalingPlugin,
        ))
        // Add all position-related systems with explicit ordering:
        // 1. sync_celestial_positions - sets positions from physics
        // 2. compute_hierarchical_scales - calculates sizes (needs positions for parent lookup)
        // 3. apply_moon_position_distortion - pushes moons outward (needs scales)
        // 4. draw_orbit_paths & draw_moon_orbit_paths - draw orbits (needs final positions)
        .add_systems(
            Update,
            (
                sync_celestial_positions,
                compute_hierarchical_scales,
                apply_moon_position_distortion,
                (draw_orbit_paths, draw_moon_orbit_paths),
            )
                .chain(),
        );
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
