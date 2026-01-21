//! Asteroid placement mode for click-to-spawn functionality.
//!
//! When placement mode is active, clicking on the viewport spawns an asteroid
//! at that location with a velocity calculated to intercept Earth.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::asteroid::{
    AsteroidCounter, calculate_velocity_for_earth_intercept, spawn_asteroid_at_position,
};
use crate::camera::{MainCamera, RENDER_SCALE};
use crate::ephemeris::Ephemeris;
use crate::types::SimulationTime;

use super::AsteroidPlacementMode;

/// System to handle clicks during asteroid placement mode.
#[allow(clippy::too_many_arguments)]
pub fn handle_asteroid_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut placement_mode: ResMut<AsteroidPlacementMode>,
    mut counter: ResMut<AsteroidCounter>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    mut contexts: EguiContexts,
) {
    // Only process when placement mode is active
    if !placement_mode.active {
        return;
    }

    // Don't interact if egui wants the pointer
    if let Some(ctx) = contexts.try_ctx_mut()
        && ctx.wants_pointer_input()
    {
        return;
    }

    // Handle click to place asteroid
    if mouse.just_pressed(MouseButton::Left) {
        let Ok(window) = window_query.get_single() else {
            return;
        };

        let Ok((camera, camera_transform)) = camera_query.get_single() else {
            return;
        };

        let Some(cursor_pos) = window.cursor_position() else {
            return;
        };

        let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
            return;
        };

        // Convert render position to physics position
        let physics_pos = DVec2::new(
            (world_pos.x as f64) / RENDER_SCALE,
            (world_pos.y as f64) / RENDER_SCALE,
        );

        // Calculate velocity for Earth intercept
        let vel = calculate_velocity_for_earth_intercept(physics_pos, &ephemeris, sim_time.current);

        info!(
            "Placing asteroid at ({:.2e}, {:.2e}) m with velocity {:.2} km/s toward Earth",
            physics_pos.x,
            physics_pos.y,
            vel.length() / 1000.0
        );

        // Spawn the asteroid
        spawn_asteroid_at_position(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut counter,
            physics_pos,
            vel,
        );

        // Exit placement mode after placing
        placement_mode.active = false;
    }

    // Cancel placement mode with right-click or escape
    if mouse.just_pressed(MouseButton::Right) {
        placement_mode.active = false;
    }
}

/// System to update cursor appearance based on placement mode.
pub fn update_placement_cursor(
    placement_mode: Res<AsteroidPlacementMode>,
    mut contexts: EguiContexts,
) {
    // Set cursor hint through egui when placement mode is active
    if placement_mode.active
        && let Some(ctx) = contexts.try_ctx_mut()
    {
        ctx.set_cursor_icon(bevy_egui::egui::CursorIcon::Crosshair);
    }
}
