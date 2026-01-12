//! Input handling for keyboard shortcuts.
//!
//! Provides keyboard controls for simulation time, camera zoom, and toggles.

use bevy::prelude::*;

use crate::camera::{MainCamera, MIN_ZOOM, MAX_ZOOM, ZOOM_SPEED};
use crate::types::SimulationTime;

/// Plugin providing keyboard input handling.
pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, keyboard_shortcuts);
    }
}

/// Handle keyboard shortcuts for simulation control.
fn keyboard_shortcuts(
    keys: Res<ButtonInput<KeyCode>>,
    mut sim_time: ResMut<SimulationTime>,
    mut camera_query: Query<&mut Projection, With<MainCamera>>,
) {
    // Space: toggle pause
    if keys.just_pressed(KeyCode::Space) {
        sim_time.paused = !sim_time.paused;
        info!("Simulation {}", if sim_time.paused { "paused" } else { "running" });
    }

    // Handle zoom with keyboard
    let Ok(mut projection) = camera_query.get_single_mut() else {
        return;
    };

    let Projection::Orthographic(ref mut ortho) = *projection else {
        return;
    };

    // Plus/Equal: zoom in (reduce scale)
    if keys.pressed(KeyCode::Equal) || keys.pressed(KeyCode::NumpadAdd) {
        let zoom_factor = 1.0 - ZOOM_SPEED;
        ortho.scale = (ortho.scale * zoom_factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    // Minus: zoom out (increase scale)
    if keys.pressed(KeyCode::Minus) || keys.pressed(KeyCode::NumpadSubtract) {
        let zoom_factor = 1.0 + ZOOM_SPEED;
        ortho.scale = (ortho.scale * zoom_factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }

    // Time controls: [ and ] to adjust simulation speed
    if keys.just_pressed(KeyCode::BracketLeft) {
        sim_time.scale = (sim_time.scale * 0.5).max(0.125);
        info!("Time scale: {}x", sim_time.scale);
    }

    if keys.just_pressed(KeyCode::BracketRight) {
        sim_time.scale = (sim_time.scale * 2.0).min(128.0);
        info!("Time scale: {}x", sim_time.scale);
    }

    // R: reset time to initial
    if keys.just_pressed(KeyCode::KeyR) {
        sim_time.reset();
        info!("Simulation reset to initial time");
    }
}
