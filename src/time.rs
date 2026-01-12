//! Time advancement system for the orbital mechanics simulator.
//!
//! Handles progression of simulation time based on scale and pause state.

use bevy::prelude::*;

use crate::types::{SimulationTime, SECONDS_PER_DAY};

/// Plugin providing time advancement functionality.
pub struct TimePlugin;

impl Plugin for TimePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, advance_time);
    }
}

/// Advance simulation time based on scale and pause state.
///
/// Time scale represents how many simulation days pass per real-world second.
/// For example, scale=1.0 means 1 sim-day per real-second.
fn advance_time(mut sim_time: ResMut<SimulationTime>, time: Res<Time>) {
    if sim_time.paused {
        return;
    }

    // delta_secs is real-world time elapsed
    // scale is how many sim-days per real-second
    // Convert to seconds: delta * scale * SECONDS_PER_DAY
    let dt = time.delta_secs_f64() * sim_time.scale * SECONDS_PER_DAY;
    sim_time.current += dt;
}
