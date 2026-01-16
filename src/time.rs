//! Time plugin for the orbital mechanics simulator.
//!
//! Note: Actual time advancement is handled by physics_step in src/physics/mod.rs.
//! This ensures simulation time is synchronized with the physics integration.
//! This plugin exists for future time-related functionality (e.g., time display formatting).

use bevy::prelude::*;

/// Plugin for time-related functionality.
///
/// Note: Simulation time advancement is handled by physics_step in FixedUpdate
/// to ensure proper synchronization between displayed time and physics state.
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, _app: &mut App) {
        // Time advancement is now handled by physics_step in src/physics/mod.rs
        // This ensures sim_time.current stays synchronized with physics integration.
        // Previously, having advance_time run on Update while physics ran on FixedUpdate
        // caused timing drift (different delta times between schedules).
    }
}
