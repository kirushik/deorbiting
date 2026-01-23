//! Rendering systems for the orbital mechanics simulator.
//!
//! This module provides visual representation of celestial bodies,
//! trajectory lines, and background elements.

mod background;
pub mod bodies;
mod deflectors;
pub mod effects;
pub mod highlight;
mod labels;
mod orbits;
pub mod scaling;
mod sync;

use bevy::prelude::*;

use self::background::BackgroundPlugin;
use self::bodies::CelestialBodyPlugin;
use self::deflectors::draw_deflector_trajectories;
use self::effects::{animate_impact_effects, spawn_impact_effects};
use self::highlight::HighlightPlugin;
use self::labels::LabelPlugin;
use self::orbits::{OrbitPathPlugin, draw_moon_orbit_paths, draw_orbit_paths};
use self::scaling::{ScalingPlugin, apply_moon_position_distortion, compute_hierarchical_scales};
use self::sync::{sync_asteroid_positions, sync_celestial_positions};

// Re-export for use in other modules
pub use self::bodies::CelestialBody;
pub use self::effects::{ImpactEffect, ImpactEffectType, SpawnImpactEffectEvent};
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
        // Register message channel for impact effects
        .init_resource::<Messages<effects::SpawnImpactEffectEvent>>()
        // Add all position-related systems with explicit ordering:
        // 1. sync_asteroid_positions - sets asteroid positions from BodyState (with distortion)
        // 2. sync_celestial_positions - sets celestial body positions from ephemeris
        // 3. compute_hierarchical_scales - calculates sizes (needs positions for parent lookup)
        // 4. apply_moon_position_distortion - pushes moons outward (needs scales)
        // 5. draw_orbit_paths & draw_moon_orbit_paths - draw orbits (needs final positions)
        // 6. spawn_impact_effects - process effect spawn events
        // 7. animate_impact_effects - render and despawn effects
        .add_systems(
            Update,
            (
                sync_asteroid_positions,
                sync_celestial_positions,
                compute_hierarchical_scales,
                apply_moon_position_distortion,
                (draw_orbit_paths, draw_moon_orbit_paths),
                draw_deflector_trajectories,
                spawn_impact_effects,
                animate_impact_effects,
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
