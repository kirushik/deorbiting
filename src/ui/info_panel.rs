//! Info panel showing selected body information and body list.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::camera::{CameraFocus, RENDER_SCALE};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::render::{CelestialBody, HoveredBody, SelectedBody};
use crate::types::{SimulationTime, AU_TO_METERS};

use super::{DisplayUnits, UiState};

/// System that renders the info panel.
pub fn info_panel(
    mut contexts: EguiContexts,
    mut selected: ResMut<SelectedBody>,
    mut hovered: ResMut<HoveredBody>,
    mut ui_state: ResMut<UiState>,
    mut camera_focus: ResMut<CameraFocus>,
    camera_query: Query<&Transform, With<crate::camera::MainCamera>>,
    bodies: Query<(Entity, &CelestialBody)>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    let panel_frame = egui::Frame::none()
        .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 220))
        .inner_margin(egui::Margin::same(12.0));

    if ui_state.info_panel_open {
        egui::SidePanel::right("info_panel")
            .resizable(false)
            .default_width(200.0)
            .frame(panel_frame)
            .show(ctx, |ui| {
                // Header with collapse button
                ui.horizontal(|ui| {
                    if ui
                        .button("\u{25C0}")
                        .on_hover_text("Collapse panel")
                        .clicked()
                    {
                        ui_state.info_panel_open = false;
                    }
                    ui.heading("Info");
                });

                ui.separator();

                // Get current camera position for focus animation
                let camera_pos = camera_query
                    .get_single()
                    .map(|t| Vec2::new(t.translation.x, t.translation.y))
                    .unwrap_or(Vec2::ZERO);

                // Body list section
                render_body_list(
                    ui,
                    &bodies,
                    &mut selected,
                    &mut hovered,
                    &ephemeris,
                    sim_time.current,
                    &mut camera_focus,
                    camera_pos,
                );

                ui.separator();

                // Selected body info
                if let Some(entity) = selected.entity {
                    if let Ok((_, body)) = bodies.get(entity) {
                        ui.heading(&body.name);
                        ui.add_space(4.0);
                        render_body_info(
                            ui,
                            body,
                            &ephemeris,
                            sim_time.current,
                            ui_state.display_units,
                        );

                        ui.add_space(8.0);

                        // Unit toggle
                        ui.horizontal(|ui| {
                            ui.label("Units:");
                            ui.selectable_value(&mut ui_state.display_units, DisplayUnits::Km, "km");
                            ui.selectable_value(&mut ui_state.display_units, DisplayUnits::Au, "AU");
                        });
                    }
                } else {
                    ui.label("Select a body from the list above.");
                }
            });
    } else {
        // Collapsed state - show expand button
        egui::SidePanel::right("info_expand")
            .resizable(false)
            .exact_width(28.0)
            .frame(panel_frame)
            .show(ctx, |ui| {
                if ui
                    .button("\u{25B6}")
                    .on_hover_text("Expand panel")
                    .clicked()
                {
                    ui_state.info_panel_open = true;
                }
            });
    }
}

/// Render the list of all celestial bodies with hover/click functionality.
/// Moons are shown indented under their parent planets.
/// Double-click on a body to center the camera on it.
fn render_body_list(
    ui: &mut egui::Ui,
    bodies: &Query<(Entity, &CelestialBody)>,
    selected: &mut ResMut<SelectedBody>,
    hovered: &mut ResMut<HoveredBody>,
    ephemeris: &Ephemeris,
    time: f64,
    camera_focus: &mut ResMut<CameraFocus>,
    camera_pos: Vec2,
) {
    ui.label("Bodies:");

    // Collect bodies by type
    let mut sun_entity = None;
    let mut planet_entities: Vec<(Entity, &CelestialBody)> = Vec::new();
    let mut moon_entities: Vec<(Entity, &CelestialBody)> = Vec::new();

    for (entity, body) in bodies.iter() {
        if body.id == CelestialBodyId::Sun {
            sun_entity = Some((entity, body));
        } else if CelestialBodyId::PLANETS.contains(&body.id) {
            planet_entities.push((entity, body));
        } else if CelestialBodyId::MOONS.contains(&body.id) {
            moon_entities.push((entity, body));
        }
    }

    // Sort planets by their ID order (roughly by distance from sun)
    planet_entities.sort_by_key(|(_, b)| {
        CelestialBodyId::PLANETS
            .iter()
            .position(|&id| id == b.id)
            .unwrap_or(999)
    });

    // Show scrollable body list (no max height constraint)
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Sun
        if let Some((entity, body)) = sun_entity {
            render_body_button(
                ui, entity, body, selected, hovered, ephemeris, time, camera_focus, camera_pos,
            );
        }

        ui.add_space(4.0);

        // Planets with their moons
        for (planet_entity, planet_body) in &planet_entities {
            render_body_button(
                ui,
                *planet_entity,
                planet_body,
                selected,
                hovered,
                ephemeris,
                time,
                camera_focus,
                camera_pos,
            );

            // Find and display moons of this planet
            let parent_id = planet_body.id;
            for (moon_entity, moon_body) in &moon_entities {
                if moon_body.id.parent() == Some(parent_id) {
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        render_body_button(
                            ui,
                            *moon_entity,
                            moon_body,
                            selected,
                            hovered,
                            ephemeris,
                            time,
                            camera_focus,
                            camera_pos,
                        );
                    });
                }
            }
        }
    });
}

/// Render a single body button with hover, click, and double-click behavior.
/// Single click selects the body, double-click centers the camera on it.
fn render_body_button(
    ui: &mut egui::Ui,
    entity: Entity,
    body: &CelestialBody,
    selected: &mut ResMut<SelectedBody>,
    hovered: &mut ResMut<HoveredBody>,
    ephemeris: &Ephemeris,
    time: f64,
    camera_focus: &mut ResMut<CameraFocus>,
    camera_pos: Vec2,
) {
    let is_selected = selected.entity == Some(entity);

    // Create button with selection highlight
    let button = egui::Button::new(&body.name).selected(is_selected);

    let response = ui.add(button);

    // Handle hover - set the hovered body in the render system
    if response.hovered() {
        hovered.entity = Some(entity);
    }

    // Handle click - select the body
    if response.clicked() {
        selected.entity = Some(entity);
    }

    // Handle double-click - center camera on body
    if response.double_clicked() {
        if let Some(pos) = ephemeris.get_position_by_id(body.id, time) {
            // Convert physics position to render position
            let render_pos = Vec2::new(
                (pos.x * RENDER_SCALE) as f32,
                (pos.y * RENDER_SCALE) as f32,
            );

            // Set camera focus target
            camera_focus.target_position = Some(render_pos);
            camera_focus.start_position = camera_pos;
            camera_focus.progress = 0.0;
        }
    }
}

fn render_body_info(
    ui: &mut egui::Ui,
    body: &CelestialBody,
    ephemeris: &Ephemeris,
    time: f64,
    units: DisplayUnits,
) {
    // Body type
    let body_type = get_body_type(body.id);
    ui.label(format!("Type: {}", body_type));

    ui.add_space(8.0);

    // Position
    let Some(pos) = ephemeris.get_position_by_id(body.id, time) else {
        ui.label("Position: unavailable");
        return;
    };

    ui.label("Position:");
    format_position(ui, pos, units);

    ui.add_space(8.0);

    // Distance from Sun
    let dist_from_sun = pos.length();
    ui.label("Distance from Sun:");
    match units {
        DisplayUnits::Km => {
            ui.label(format!("  {:.2} M km", dist_from_sun / 1e9));
        }
        DisplayUnits::Au => {
            ui.label(format!("  {:.4} AU", dist_from_sun / AU_TO_METERS));
        }
    }

    // For moons, show parent info
    if let Some(parent) = body.id.parent() {
        ui.add_space(8.0);
        ui.label(format!("Orbits: {}", parent.name()));
    }
}

fn get_body_type(id: CelestialBodyId) -> &'static str {
    if id == CelestialBodyId::Sun {
        "Star"
    } else if CelestialBodyId::PLANETS.contains(&id) {
        "Planet"
    } else if CelestialBodyId::MOONS.contains(&id) {
        "Moon"
    } else {
        "Unknown"
    }
}

fn format_position(ui: &mut egui::Ui, pos: DVec2, units: DisplayUnits) {
    match units {
        DisplayUnits::Km => {
            ui.label(format!("  X: {:.2} M km", pos.x / 1e9));
            ui.label(format!("  Y: {:.2} M km", pos.y / 1e9));
        }
        DisplayUnits::Au => {
            ui.label(format!("  X: {:.4} AU", pos.x / AU_TO_METERS));
            ui.label(format!("  Y: {:.4} AU", pos.y / AU_TO_METERS));
        }
    }
}
