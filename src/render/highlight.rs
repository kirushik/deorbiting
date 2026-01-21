//! Hover and selection highlighting for celestial bodies and asteroids.
//!
//! Provides visual feedback when the mouse hovers over or selects interactive elements.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiContexts;

use crate::asteroid::{Asteroid, AsteroidVisual};
use crate::camera::MainCamera;
use crate::input::DragState;
use crate::render::bodies::CelestialBody;
use crate::render::z_layers;
use crate::types::{InputSystemSet, SelectableBody};
use crate::ui::velocity_handle::VelocityDragState;

/// Plugin providing hover and selection highlighting.
pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredBody>()
            .init_resource::<SelectedBody>()
            .add_systems(
                Update,
                (
                    detect_hover,
                    detect_selection,
                    draw_persistent_asteroid_markers,
                    draw_hover_highlight,
                    draw_selection_highlight,
                )
                    .chain()
                    // Run after velocity drag so it can check if drag started
                    .after(InputSystemSet::VelocityDrag),
            );
    }
}

/// Resource tracking the currently hovered body.
#[derive(Resource, Default)]
pub struct HoveredBody {
    /// The currently hovered body, if any.
    pub body: Option<SelectableBody>,
}

/// Resource tracking the currently selected body.
#[derive(Resource, Default)]
pub struct SelectedBody {
    /// The currently selected body, if any.
    pub body: Option<SelectableBody>,
}

/// Find the body under the cursor, if any.
///
/// Checks both celestial bodies and asteroids, returning the closest one.
fn find_body_at_cursor(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
    celestial_bodies: &Query<(Entity, &Transform, &CelestialBody)>,
    asteroids: &Query<(Entity, &Transform, &AsteroidVisual), With<Asteroid>>,
) -> Option<SelectableBody> {
    let cursor_pos = window.cursor_position()?;

    // Convert cursor position to world coordinates
    let world_pos = camera
        .viewport_to_world_2d(camera_transform, cursor_pos)
        .ok()?;

    // Find the closest body under the cursor (checking both types)
    let mut closest: Option<(SelectableBody, f32)> = None;

    // Check celestial bodies
    for (entity, transform, body) in celestial_bodies.iter() {
        let body_pos = transform.translation.truncate();
        let dist = (world_pos - body_pos).length();

        // Calculate hit radius based on visual size
        // Use a generous hit area for easier selection
        let base_radius = (body.radius * crate::camera::RENDER_SCALE) as f32 * body.visual_scale;
        let hit_radius = base_radius.max(2.0) * 2.0; // At least 2 units, doubled for easier picking

        if dist < hit_radius && closest.is_none_or(|(_, d)| dist < d) {
            closest = Some((SelectableBody::Celestial(entity), dist));
        }
    }

    // Check asteroids
    for (entity, transform, visual) in asteroids.iter() {
        let asteroid_pos = transform.translation.truncate();
        let dist = (world_pos - asteroid_pos).length();

        // Use asteroid visual radius for hit detection
        let hit_radius = visual.render_radius.max(2.0) * 2.0;

        if dist < hit_radius && closest.is_none_or(|(_, d)| dist < d) {
            closest = Some((SelectableBody::Asteroid(entity), dist));
        }
    }

    closest.map(|(body, _)| body)
}

/// Detect which body (celestial or asteroid) the mouse is hovering over.
fn detect_hover(
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    celestial_bodies: Query<(Entity, &Transform, &CelestialBody)>,
    asteroids: Query<(Entity, &Transform, &AsteroidVisual), With<Asteroid>>,
    mut hovered: ResMut<HoveredBody>,
    mut contexts: EguiContexts,
    drag_state: Res<DragState>,
    velocity_drag_state: Res<VelocityDragState>,
) {
    // Don't detect hover while dragging an asteroid (position or velocity)
    if drag_state.dragging.is_some() || velocity_drag_state.dragging {
        return;
    }

    // Don't detect hover if egui wants the pointer
    if let Some(ctx) = contexts.try_ctx_mut()
        && ctx.wants_pointer_input()
    {
        hovered.body = None;
        return;
    }

    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    hovered.body = find_body_at_cursor(
        window,
        camera,
        camera_transform,
        &celestial_bodies,
        &asteroids,
    );
}

/// Detect clicks on bodies (celestial or asteroid) to select them.
fn detect_selection(
    mouse: Res<ButtonInput<MouseButton>>,
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    celestial_bodies: Query<(Entity, &Transform, &CelestialBody)>,
    asteroids: Query<(Entity, &Transform, &AsteroidVisual), With<Asteroid>>,
    mut selected: ResMut<SelectedBody>,
    mut contexts: EguiContexts,
    drag_state: Res<DragState>,
    velocity_drag_state: Res<VelocityDragState>,
) {
    // Don't change selection while dragging an asteroid (position or velocity)
    if drag_state.dragging.is_some() || velocity_drag_state.dragging {
        return;
    }

    // Only process on left click
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    // Don't process click if egui wants the pointer (clicking on UI)
    if let Some(ctx) = contexts.try_ctx_mut()
        && ctx.wants_pointer_input()
    {
        return;
    }

    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    // Find body at cursor and select it (or deselect if clicking empty space)
    selected.body = find_body_at_cursor(
        window,
        camera,
        camera_transform,
        &celestial_bodies,
        &asteroids,
    );
}

/// Draw highlight ring around hovered body.
fn draw_hover_highlight(
    mut gizmos: Gizmos,
    hovered: Res<HoveredBody>,
    selected: Res<SelectedBody>,
    celestial_bodies: Query<(&Transform, &CelestialBody)>,
    asteroids: Query<(&Transform, &AsteroidVisual), With<Asteroid>>,
) {
    let Some(hovered_body) = hovered.body else {
        return;
    };

    // Don't draw hover highlight if this body is already selected
    // (selection highlight takes precedence)
    if selected.body == Some(hovered_body) {
        return;
    }

    // Cyan color for hover
    let color = Color::srgba(0.0, 1.0, 1.0, 0.8);

    match hovered_body {
        SelectableBody::Celestial(entity) => {
            if let Ok((transform, body)) = celestial_bodies.get(entity) {
                let radius = (body.radius * crate::camera::RENDER_SCALE) as f32 * body.visual_scale;
                draw_highlight_ring_at(
                    &mut gizmos,
                    transform.translation.truncate(),
                    radius,
                    color,
                );
            }
        }
        SelectableBody::Asteroid(entity) => {
            if let Ok((transform, visual)) = asteroids.get(entity) {
                draw_highlight_ring_at(
                    &mut gizmos,
                    transform.translation.truncate(),
                    visual.render_radius,
                    color,
                );
            }
        }
    }
}

/// Draw highlight ring around selected body.
fn draw_selection_highlight(
    mut gizmos: Gizmos,
    selected: Res<SelectedBody>,
    celestial_bodies: Query<(&Transform, &CelestialBody)>,
    asteroids: Query<(&Transform, &AsteroidVisual), With<Asteroid>>,
) {
    let Some(selected_body) = selected.body else {
        return;
    };

    // Gold/yellow for selection (more prominent than hover)
    let color = Color::srgba(1.0, 0.85, 0.0, 1.0);

    match selected_body {
        SelectableBody::Celestial(entity) => {
            if let Ok((transform, body)) = celestial_bodies.get(entity) {
                let radius = (body.radius * crate::camera::RENDER_SCALE) as f32 * body.visual_scale;
                draw_highlight_ring_at(
                    &mut gizmos,
                    transform.translation.truncate(),
                    radius,
                    color,
                );
            }
        }
        SelectableBody::Asteroid(entity) => {
            if let Ok((transform, visual)) = asteroids.get(entity) {
                draw_highlight_ring_at(
                    &mut gizmos,
                    transform.translation.truncate(),
                    visual.render_radius,
                    color,
                );
            }
        }
    }
}

/// Draw a highlight ring at a position with a given radius.
fn draw_highlight_ring_at(gizmos: &mut Gizmos, center_2d: Vec2, base_radius: f32, color: Color) {
    let ring_radius = base_radius.max(1.0) * 1.5;

    // Draw ring at UI layer (above celestial bodies and asteroids)
    let center = Vec3::new(center_2d.x, center_2d.y, z_layers::UI_HANDLES);

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

/// Draw persistent visibility markers for ALL asteroids.
/// These markers ensure asteroids remain visible at any zoom level,
/// even when not selected or hovered. Uses each asteroid's unique indicator color.
fn draw_persistent_asteroid_markers(
    mut gizmos: Gizmos,
    asteroids: Query<(Entity, &Transform, &AsteroidVisual), With<Asteroid>>,
    selected: Res<SelectedBody>,
    hovered: Res<HoveredBody>,
) {
    use crate::asteroid::indicator_color_from_material;

    for (entity, transform, visual) in asteroids.iter() {
        // Skip if this asteroid is selected or hovered (those have their own highlights)
        let is_selected = matches!(selected.body, Some(SelectableBody::Asteroid(e)) if e == entity);
        let is_hovered = matches!(hovered.body, Some(SelectableBody::Asteroid(e)) if e == entity);

        if is_selected || is_hovered {
            continue;
        }

        // Get the vibrant indicator color for this asteroid (with reduced alpha)
        let indicator = indicator_color_from_material(visual.color);
        let marker_color = indicator.with_alpha(0.5);

        let center_2d = transform.translation.truncate();

        // Use actual scaled size from transform (matches what's rendered)
        // Add 50% padding for visibility ring
        let actual_radius = visual.render_radius * transform.scale.x;
        let marker_radius = actual_radius.max(1.5) * 1.5;

        // Draw the marker ring at UI layer
        let center = Vec3::new(center_2d.x, center_2d.y, z_layers::UI_HANDLES - 0.1);

        // Use same segment count as selection highlight for consistency
        let segments = 32;
        for i in 0..segments {
            let t0 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let t1 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

            let p0 = center + Vec3::new(marker_radius * t0.cos(), marker_radius * t0.sin(), 0.0);
            let p1 = center + Vec3::new(marker_radius * t1.cos(), marker_radius * t1.sin(), 0.0);

            gizmos.line(p0, p1, marker_color);
        }
    }
}
