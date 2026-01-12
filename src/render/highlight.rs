//! Hover highlighting for celestial bodies.
//!
//! Provides visual feedback when the mouse hovers over interactive elements.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::camera::MainCamera;
use crate::render::bodies::CelestialBody;
use crate::render::z_layers;

/// Plugin providing hover highlighting.
pub struct HighlightPlugin;

impl Plugin for HighlightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HoveredBody>()
            .add_systems(Update, (detect_hover, draw_highlight).chain());
    }
}

/// Resource tracking the currently hovered body.
#[derive(Resource, Default)]
pub struct HoveredBody {
    /// Entity of the currently hovered body, if any.
    pub entity: Option<Entity>,
}

/// Detect which celestial body the mouse is hovering over.
fn detect_hover(
    window_query: Query<&Window, With<PrimaryWindow>>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    bodies: Query<(Entity, &Transform, &CelestialBody)>,
    mut hovered: ResMut<HoveredBody>,
) {
    let Ok(window) = window_query.get_single() else {
        return;
    };

    let Some(cursor_pos) = window.cursor_position() else {
        hovered.entity = None;
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    // Convert cursor position to world coordinates
    let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) else {
        hovered.entity = None;
        return;
    };

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
            if closest.map_or(true, |(_, d)| dist < d) {
                closest = Some((entity, dist));
            }
        }
    }

    hovered.entity = closest.map(|(e, _)| e);
}

/// Draw highlight ring around hovered body.
fn draw_highlight(
    mut gizmos: Gizmos,
    hovered: Res<HoveredBody>,
    bodies: Query<(&Transform, &CelestialBody)>,
) {
    let Some(entity) = hovered.entity else {
        return;
    };

    let Ok((transform, body)) = bodies.get(entity) else {
        return;
    };

    // Calculate highlight ring radius
    let base_radius = (body.radius * crate::camera::RENDER_SCALE) as f32 * body.visual_scale;
    let ring_radius = base_radius.max(1.0) * 1.5;

    // Draw cyan highlight ring
    let center = Vec3::new(
        transform.translation.x,
        transform.translation.y,
        z_layers::UI_HANDLES, // Above celestial bodies
    );

    let color = Color::srgba(0.0, 1.0, 1.0, 0.8); // Cyan

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
