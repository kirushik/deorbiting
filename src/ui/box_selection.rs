//! Box selection - drag to select asteroids within a rectangle.
//!
//! Click and drag on empty space to draw a selection box.
//! All asteroids within the box will be considered for selection.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiContexts, egui};

use crate::asteroid::{Asteroid, AsteroidVisual};
use crate::camera::MainCamera;
use crate::input::DragState;
use crate::render::SelectedBody;
use crate::types::SelectableBody;
use crate::ui::velocity_handle::VelocityDragState;

/// Resource tracking box selection state.
#[derive(Resource, Default)]
pub struct BoxSelectionState {
    /// Whether box selection is currently active.
    pub active: bool,
    /// Starting position in screen coordinates.
    pub start_screen: Vec2,
    /// Current position in screen coordinates.
    pub current_screen: Vec2,
    /// Starting position in world coordinates.
    pub start_world: Vec2,
    /// Current position in world coordinates.
    pub current_world: Vec2,
}

impl BoxSelectionState {
    /// Get the selection rectangle in screen coordinates.
    pub fn screen_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(
            self.start_screen.x.min(self.current_screen.x),
            self.start_screen.y.min(self.current_screen.y),
        );
        let max = Vec2::new(
            self.start_screen.x.max(self.current_screen.x),
            self.start_screen.y.max(self.current_screen.y),
        );
        (min, max)
    }

    /// Get the selection rectangle in world coordinates.
    pub fn world_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(
            self.start_world.x.min(self.current_world.x),
            self.start_world.y.min(self.current_world.y),
        );
        let max = Vec2::new(
            self.start_world.x.max(self.current_world.x),
            self.start_world.y.max(self.current_world.y),
        );
        (min, max)
    }
}

/// System to handle box selection input.
#[allow(clippy::too_many_arguments)]
pub fn box_selection_input(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut box_state: ResMut<BoxSelectionState>,
    mut selected: ResMut<SelectedBody>,
    asteroids: Query<(Entity, &Transform, &AsteroidVisual), With<Asteroid>>,
    mut contexts: EguiContexts,
    drag_state: Res<DragState>,
    velocity_drag_state: Res<VelocityDragState>,
) {
    // Don't start box selection while dragging an asteroid
    if drag_state.dragging.is_some() || velocity_drag_state.dragging {
        return;
    }

    let Ok(window) = window_query.single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.single() else {
        return;
    };

    let Some(cursor_screen) = window.cursor_position() else {
        // If cursor left window while selecting, cancel
        if box_state.active && mouse.just_released(MouseButton::Left) {
            box_state.active = false;
        }
        return;
    };

    // Convert screen position to world position
    let cursor_world = camera
        .viewport_to_world_2d(camera_transform, cursor_screen)
        .unwrap_or(Vec2::ZERO);

    // Start box selection on left mouse button press (only if not over UI)
    if mouse.just_pressed(MouseButton::Left) && !box_state.active {
        // Don't start if egui wants the pointer
        if let Ok(ctx) = contexts.ctx_mut()
            && ctx.wants_pointer_input()
        {
            return;
        }

        // Check if clicking on empty space (not on an asteroid)
        let click_on_asteroid = asteroids.iter().any(|(_, transform, visual)| {
            let asteroid_pos = transform.translation.truncate();
            let distance = (asteroid_pos - cursor_world).length();
            distance < visual.render_radius * 2.0 // Give some margin for clicking
        });

        if !click_on_asteroid {
            // Start box selection
            box_state.active = true;
            box_state.start_screen = cursor_screen;
            box_state.current_screen = cursor_screen;
            box_state.start_world = cursor_world;
            box_state.current_world = cursor_world;
        }
    }

    // Update box selection while dragging
    if box_state.active && mouse.pressed(MouseButton::Left) {
        box_state.current_screen = cursor_screen;
        box_state.current_world = cursor_world;
    }

    // Complete box selection on release
    if box_state.active && mouse.just_released(MouseButton::Left) {
        box_state.active = false;

        // Only select if the box has some size (avoid accidental clicks)
        let (min, max) = box_state.screen_rect();
        let box_size = max - min;
        if box_size.x > 5.0 && box_size.y > 5.0 {
            // Find asteroids in the box
            let (world_min, world_max) = box_state.world_rect();

            let mut asteroids_in_box: Vec<(Entity, f32)> = asteroids
                .iter()
                .filter_map(|(entity, transform, _)| {
                    let pos = transform.translation.truncate();
                    if pos.x >= world_min.x
                        && pos.x <= world_max.x
                        && pos.y >= world_min.y
                        && pos.y <= world_max.y
                    {
                        // Calculate distance to box center
                        let box_center = (world_min + world_max) / 2.0;
                        let dist = (pos - box_center).length();
                        // Filter out NaN distances
                        if dist.is_finite() {
                            Some((entity, dist))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            // Select the asteroid closest to the center of the box
            if !asteroids_in_box.is_empty() {
                // Sort with NaN-safe comparison
                asteroids_in_box
                    .sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                selected.body = Some(SelectableBody::Asteroid(asteroids_in_box[0].0));
            }
        }
    }

    // Cancel on right-click or escape
    if box_state.active
        && (mouse.just_pressed(MouseButton::Right)
            || contexts
                .ctx_mut()
                .ok()
                .map(|ctx| ctx.input(|i| i.key_pressed(egui::Key::Escape)))
                .unwrap_or(false))
    {
        box_state.active = false;
    }
}

/// System to render the selection box.
pub fn render_box_selection(mut gizmos: Gizmos, box_state: Res<BoxSelectionState>) {
    if !box_state.active {
        return;
    }

    let (min, max) = box_state.world_rect();
    let size = max - min;
    let center = (min + max) / 2.0;

    // Draw selection box outline
    let color = Color::srgba(0.4, 0.7, 1.0, 0.8);
    gizmos.rect_2d(Isometry2d::from_translation(center), size, color);

    // Draw semi-transparent fill
    let fill_color = Color::srgba(0.4, 0.7, 1.0, 0.15);
    gizmos.rect_2d(
        Isometry2d::from_translation(center),
        size * 0.99, // Slightly smaller to not overlap with outline
        fill_color,
    );
}
