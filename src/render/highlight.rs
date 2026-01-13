//! Hover and selection highlighting for celestial bodies.
//!
//! Provides visual feedback when the mouse hovers over or selects interactive elements.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::camera::MainCamera;
use crate::render::bodies::CelestialBody;
use crate::render::z_layers;

/// Plugin providing hover and selection highlighting.
pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredBody>()
            .init_resource::<SelectedBody>()
            .add_systems(
                Update,
                (detect_hover, detect_selection, draw_hover_highlight, draw_selection_highlight).chain(),
            );
    }
}

/// Resource tracking the currently hovered body.
#[derive(Resource, Default)]
pub struct HoveredBody {
    /// Entity of the currently hovered body, if any.
    pub entity: Option<Entity>,
}

/// Resource tracking the currently selected body.
#[derive(Resource, Default)]
pub struct SelectedBody {
    /// Entity of the currently selected body, if any.
    pub entity: Option<Entity>,
}

/// Find the body under the cursor, if any.
fn find_body_at_cursor(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    bodies: &Query<(Entity, &Transform, &CelestialBody)>,
) -> Option<Entity> {
    let cursor_pos = window.cursor_position()?;

    // Convert cursor position to world coordinates
    let world_pos = camera.viewport_to_world_2d(camera_transform, cursor_pos).ok()?;

    // Find the closest body under the cursor
    let mut closest: Option<(Entity, f32)> = None;

    for (entity, transform, body) in bodies.iter() {
        let body_pos = transform.translation.truncate();
        let dist = (world_pos - body_pos).length();

        // Calculate hit radius based on visual size
        // Use a generous hit area for easier selection
        let base_radius = (body.radius * crate::camera::RENDER_SCALE) as f32 * body.visual_scale;
        let hit_radius = base_radius.max(2.0) * 2.0; // At least 2 units, doubled for easier picking

        if dist < hit_radius {
            if closest.is_none_or(|(_, d)| dist < d) {
                closest = Some((entity, dist));
            }
        }
    }

    closest.map(|(e, _)| e)
}

/// Detect which celestial body the mouse is hovering over.
fn detect_hover(
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    bodies: Query<(Entity, &Transform, &CelestialBody)>,
    mut hovered: ResMut<HoveredBody>,
    mut contexts: EguiContexts,
) {
    // Don't detect hover if egui wants the pointer
    if let Some(ctx) = contexts.try_ctx_mut() {
        if ctx.wants_pointer_input() {
            hovered.entity = None;
            return;
        }
    }

    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    hovered.entity = find_body_at_cursor(window, camera, camera_transform, &bodies);
}

/// Detect clicks on celestial bodies to select them.
fn detect_selection(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    bodies: Query<(Entity, &Transform, &CelestialBody)>,
    mut selected: ResMut<SelectedBody>,
    mut contexts: EguiContexts,
) {
    // Only process on left click
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    // Don't process click if egui wants the pointer (clicking on UI)
    if let Some(ctx) = contexts.try_ctx_mut() {
        if ctx.wants_pointer_input() {
            return;
        }
    }

    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    // Find body at cursor and select it (or deselect if clicking empty space)
    selected.entity = find_body_at_cursor(window, camera, camera_transform, &bodies);
}

/// Draw highlight ring around hovered body.
fn draw_hover_highlight(
    mut gizmos: Gizmos,
    hovered: Res<HoveredBody>,
    selected: Res<SelectedBody>,
    bodies: Query<(&Transform, &CelestialBody)>,
) {
    let Some(entity) = hovered.entity else {
        return;
    };

    // Don't draw hover highlight if this body is already selected
    // (selection highlight takes precedence)
    if selected.entity == Some(entity) {
        return;
    }

    let Ok((transform, body)) = bodies.get(entity) else {
        return;
    };

    draw_highlight_ring(&mut gizmos, transform, body, Color::srgba(0.0, 1.0, 1.0, 0.8)); // Cyan
}

/// Draw highlight ring around selected body.
fn draw_selection_highlight(
    mut gizmos: Gizmos,
    selected: Res<SelectedBody>,
    bodies: Query<(&Transform, &CelestialBody)>,
) {
    let Some(entity) = selected.entity else {
        return;
    };

    let Ok((transform, body)) = bodies.get(entity) else {
        return;
    };

    // Gold/yellow for selection (more prominent than hover)
    draw_highlight_ring(&mut gizmos, transform, body, Color::srgba(1.0, 0.85, 0.0, 1.0));
}

/// Draw a highlight ring around a body.
fn draw_highlight_ring(gizmos: &mut Gizmos, transform: &Transform, body: &CelestialBody, color: Color) {
    // Calculate highlight ring radius
    let base_radius = (body.radius * crate::camera::RENDER_SCALE) as f32 * body.visual_scale;
    let ring_radius = base_radius.max(1.0) * 1.5;

    // Draw ring at UI layer (above celestial bodies)
    let center = Vec3::new(
        transform.translation.x,
        transform.translation.y,
        z_layers::UI_HANDLES,
    );

    // Draw circle using line segments
    let segments = 32;
    for i in 0..segments {
        let t0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let t1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

        let p0 = center + Vec3::new(ring_radius * t0.cos(), ring_radius * t0.sin(), 0.0);
        let p1 = center + Vec3::new(ring_radius * t1.cos(), ring_radius * t1.sin(), 0.0);

        gizmos.line(p0, p1, color);
    }
}
