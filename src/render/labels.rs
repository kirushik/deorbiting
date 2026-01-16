//! Body labels using egui for text rendering.
//!
//! Renders planet and moon names near each celestial body.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::camera::MainCamera;
use crate::ephemeris::data::CelestialBodyId;
use crate::render::bodies::CelestialBody;

/// Plugin providing body label rendering.
pub struct LabelPlugin;

impl Plugin for LabelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LabelSettings>()
            .add_systems(Update, draw_body_labels);
    }
}

/// Settings for label rendering.
#[derive(Resource)]
pub struct LabelSettings {
    /// Whether labels are visible.
    pub visible: bool,
    /// Maximum zoom level to show planet labels (hide when too zoomed out).
    pub max_zoom_for_planets: f32,
    /// Maximum zoom level to show moon labels (much tighter - moons only visible when zoomed in).
    pub max_zoom_for_moons: f32,
    /// Offset from body center in screen pixels.
    pub offset: f32,
}

impl Default for LabelSettings {
    fn default() -> Self {
        Self {
            visible: true,
            max_zoom_for_planets: 50.0,
            max_zoom_for_moons: 0.1, // Only show moon labels when very zoomed in
            offset: 15.0,
        }
    }
}

/// Check if a body ID represents a moon.
fn is_moon(id: CelestialBodyId) -> bool {
    matches!(
        id,
        CelestialBodyId::Moon
            | CelestialBodyId::Phobos
            | CelestialBodyId::Deimos
            | CelestialBodyId::Io
            | CelestialBodyId::Europa
            | CelestialBodyId::Ganymede
            | CelestialBodyId::Callisto
            | CelestialBodyId::Titan
            | CelestialBodyId::Enceladus
    )
}

/// Draw labels for all celestial bodies.
fn draw_body_labels(
    mut egui_ctx: EguiContexts,
    bodies: Query<(&CelestialBody, &Transform)>,
    camera: Query<(&Camera, &GlobalTransform, &Projection), With<MainCamera>>,
    settings: Res<LabelSettings>,
) {
    if !settings.visible {
        return;
    }

    let Ok((camera, camera_transform, projection)) = camera.get_single() else {
        return;
    };

    // Get current zoom level
    let zoom = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => return,
    };

    egui::Area::new(egui::Id::new("body_labels"))
        .fixed_pos(egui::pos2(0.0, 0.0))
        .order(egui::Order::Background)
        .show(egui_ctx.ctx_mut(), |ui| {
            let painter = ui.painter();

            for (body, transform) in bodies.iter() {
                // Skip labels based on zoom level and body type
                let is_moon_body = is_moon(body.id);
                let max_zoom = if is_moon_body {
                    settings.max_zoom_for_moons
                } else {
                    settings.max_zoom_for_planets
                };

                if zoom > max_zoom {
                    continue;
                }

                let world_pos = transform.translation;

                // Project world position to screen
                let Ok(screen_pos) = camera.world_to_viewport(camera_transform, world_pos) else {
                    continue;
                };

                // Offset label slightly below and to the right of the body
                let label_pos = egui::pos2(
                    screen_pos.x + settings.offset,
                    screen_pos.y + settings.offset,
                );

                // Draw text with shadow for readability
                let text = &body.name;
                let font = egui::FontId::proportional(14.0);

                // Shadow
                painter.text(
                    label_pos + egui::vec2(1.0, 1.0),
                    egui::Align2::LEFT_TOP,
                    text,
                    font.clone(),
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 180),
                );

                // Main text
                painter.text(
                    label_pos,
                    egui::Align2::LEFT_TOP,
                    text,
                    font,
                    egui::Color32::from_rgba_unmultiplied(220, 220, 220, 230),
                );
            }
        });
}
