//! Info panel showing selected body information and body list.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asteroid::{Asteroid, AsteroidName};
use crate::camera::{CameraFocus, MainCamera, RENDER_SCALE};
use crate::input::DragState;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::render::{CelestialBody, HoveredBody, SelectedBody};
use crate::types::{BodyState, SelectableBody, SimulationTime, AU_TO_METERS};

use super::interceptor_launch::InterceptorLaunchState;
use super::{AsteroidPlacementMode, DisplayUnits, TogglePlacementModeEvent, UiState};

/// System that renders the info panel.
#[allow(clippy::too_many_arguments)]
pub fn info_panel(
    mut commands: Commands,
    mut contexts: EguiContexts,
    mut selected: ResMut<SelectedBody>,
    mut hovered: ResMut<HoveredBody>,
    mut ui_state: ResMut<UiState>,
    mut camera_focus: ResMut<CameraFocus>,
    mut integrator_states: ResMut<IntegratorStates>,
    camera_query: Query<(&Transform, &Camera, &GlobalTransform), With<MainCamera>>,
    celestial_bodies: Query<(Entity, &CelestialBody)>,
    asteroids: Query<(Entity, &AsteroidName, &BodyState), With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    drag_state: Res<DragState>,
    placement_mode: Res<AsteroidPlacementMode>,
    mut toggle_placement: EventWriter<TogglePlacementModeEvent>,
    mut launch_state: ResMut<InterceptorLaunchState>,
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
            .default_width(220.0)
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
                let (camera_transform, _camera, _camera_global) = camera_query.get_single().unwrap();
                let camera_pos = Vec2::new(camera_transform.translation.x, camera_transform.translation.y);

                // Celestial body list section
                render_body_list(
                    ui,
                    &celestial_bodies,
                    &mut selected,
                    &mut hovered,
                    &ephemeris,
                    sim_time.current,
                    &mut camera_focus,
                    camera_pos,
                    &drag_state,
                );

                ui.separator();

                // Asteroid section
                let spawn_clicked = render_asteroid_section(
                    ui,
                    &mut commands,
                    &asteroids,
                    &mut selected,
                    &mut hovered,
                    &mut integrator_states,
                    &mut camera_focus,
                    camera_pos,
                    &drag_state,
                    placement_mode.active,
                    &mut launch_state,
                );

                // Send event to toggle placement mode
                if spawn_clicked {
                    toggle_placement.send(TogglePlacementModeEvent);
                }

                ui.separator();

                // Selected body info
                render_selected_info(
                    ui,
                    &selected,
                    &celestial_bodies,
                    &asteroids,
                    &ephemeris,
                    sim_time.current,
                    &mut ui_state,
                );
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
    drag_state: &Res<DragState>,
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
                ui, entity, body, selected, hovered, ephemeris, time, camera_focus, camera_pos, drag_state,
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
                drag_state,
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
                            drag_state,
                        );
                    });
                }
            }
        }
    });
}

/// Render a single celestial body button with hover, click, and double-click behavior.
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
    drag_state: &Res<DragState>,
) {
    let selectable = SelectableBody::Celestial(entity);
    let is_selected = selected.body == Some(selectable);

    // Create button with selection highlight
    let button = egui::Button::new(&body.name).selected(is_selected);

    let response = ui.add(button);

    // Don't change hover/selection while dragging an asteroid
    if drag_state.dragging.is_some() {
        return;
    }

    // Handle hover - set the hovered body in the render system
    if response.hovered() {
        hovered.body = Some(selectable);
    }

    // Handle click - select the body
    if response.clicked() {
        selected.body = Some(selectable);
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

/// Render the asteroid section with spawn button, list, and edit controls.
///
/// Returns true if the spawn button was clicked (to trigger placement mode).
#[allow(clippy::too_many_arguments)]
fn render_asteroid_section(
    ui: &mut egui::Ui,
    commands: &mut Commands,
    asteroids: &Query<(Entity, &AsteroidName, &BodyState), With<Asteroid>>,
    selected: &mut ResMut<SelectedBody>,
    hovered: &mut ResMut<HoveredBody>,
    integrator_states: &mut ResMut<IntegratorStates>,
    camera_focus: &mut ResMut<CameraFocus>,
    camera_pos: Vec2,
    drag_state: &Res<DragState>,
    placement_mode_active: bool,
    launch_state: &mut ResMut<InterceptorLaunchState>,
) -> bool {
    let mut spawn_clicked = false;

    ui.collapsing("Asteroids", |ui| {
        // Spawn button - enters placement mode for click-to-spawn
        let button_text = if placement_mode_active {
            "✕ Cancel Placement"
        } else {
            "+ Spawn Asteroid"
        };

        let button = egui::Button::new(button_text);
        let button = if placement_mode_active {
            button.fill(egui::Color32::from_rgb(180, 80, 80))
        } else {
            button
        };

        if ui.add(button).clicked() {
            spawn_clicked = true;
        }

        if placement_mode_active {
            ui.label(egui::RichText::new("Click on map to place").italics().small());
        }

        ui.add_space(4.0);

        // List asteroids
        let mut asteroid_list: Vec<_> = asteroids.iter().collect();
        asteroid_list.sort_by(|a, b| a.1 .0.cmp(&b.1 .0)); // Sort by name

        for (entity, name, body_state) in asteroid_list {
            let selectable = SelectableBody::Asteroid(entity);
            let is_selected = selected.body == Some(selectable);

            let button = egui::Button::new(&name.0).selected(is_selected);
            let response = ui.add(button);

            // Don't change hover/selection while dragging an asteroid
            if drag_state.dragging.is_none() {
                if response.hovered() {
                    hovered.body = Some(selectable);
                }

                if response.clicked() {
                    selected.body = Some(selectable);
                }

                // Double-click to focus
                if response.double_clicked() {
                    let render_pos = Vec2::new(
                        (body_state.pos.x * RENDER_SCALE) as f32,
                        (body_state.pos.y * RENDER_SCALE) as f32,
                    );
                    camera_focus.target_position = Some(render_pos);
                    camera_focus.start_position = camera_pos;
                    camera_focus.progress = 0.0;
                }
            }
        }

        // Edit controls for selected asteroid
        if let Some(SelectableBody::Asteroid(entity)) = selected.body {
            if asteroids.get(entity).is_ok() {
                ui.add_space(8.0);
                ui.separator();
                ui.label("Selected Asteroid:");

                // Delete button
                if ui.button("Delete Asteroid").clicked() {
                    commands.entity(entity).despawn();
                    integrator_states.remove(entity);
                    selected.body = None;
                    if hovered.body == Some(SelectableBody::Asteroid(entity)) {
                        hovered.body = None;
                    }
                }

                // Launch interceptor button
                if ui.button("Launch Interceptor").clicked() {
                    launch_state.open = true;
                }
            }
        }
    });

    spawn_clicked
}

/// Render information about the currently selected body.
fn render_selected_info(
    ui: &mut egui::Ui,
    selected: &ResMut<SelectedBody>,
    celestial_bodies: &Query<(Entity, &CelestialBody)>,
    asteroids: &Query<(Entity, &AsteroidName, &BodyState), With<Asteroid>>,
    ephemeris: &Ephemeris,
    time: f64,
    ui_state: &mut ResMut<UiState>,
) {
    match selected.body {
        Some(SelectableBody::Celestial(entity)) => {
            if let Ok((_, body)) = celestial_bodies.get(entity) {
                ui.heading(&body.name);
                ui.add_space(4.0);
                render_body_info(ui, body, ephemeris, time, ui_state.display_units);

                ui.add_space(8.0);

                // Unit toggle
                ui.horizontal(|ui| {
                    ui.label("Units:");
                    ui.selectable_value(&mut ui_state.display_units, DisplayUnits::Km, "km");
                    ui.selectable_value(&mut ui_state.display_units, DisplayUnits::Au, "AU");
                });
            }
        }
        Some(SelectableBody::Asteroid(entity)) => {
            if let Ok((_, name, body_state)) = asteroids.get(entity) {
                ui.heading(&name.0);
                ui.add_space(4.0);
                render_asteroid_info(ui, body_state, ui_state.display_units);

                ui.add_space(8.0);

                // Unit toggle
                ui.horizontal(|ui| {
                    ui.label("Units:");
                    ui.selectable_value(&mut ui_state.display_units, DisplayUnits::Km, "km");
                    ui.selectable_value(&mut ui_state.display_units, DisplayUnits::Au, "AU");
                });
            }
        }
        None => {
            ui.label("Select a body from the list above.");
        }
    }
}

/// Render info for a selected asteroid.
fn render_asteroid_info(ui: &mut egui::Ui, body_state: &BodyState, units: DisplayUnits) {
    ui.label("Type: Asteroid");

    ui.add_space(8.0);

    // Position
    ui.label("Position:");
    format_position(ui, body_state.pos, units);

    ui.add_space(8.0);

    // Velocity
    ui.label("Velocity:");
    let vel_km_s = body_state.vel_km_per_s();
    ui.label(format!("  {:.2} km/s", vel_km_s.length()));

    ui.add_space(8.0);

    // Distance from Sun
    let dist_from_sun = body_state.pos.length();
    ui.label("Distance from Sun:");
    match units {
        DisplayUnits::Km => {
            ui.label(format!("  {:.2} M km", dist_from_sun / 1e9));
        }
        DisplayUnits::Au => {
            ui.label(format!("  {:.4} AU", dist_from_sun / AU_TO_METERS));
        }
    }

    ui.add_space(8.0);

    // Mass
    ui.label(format!("Mass: {:.2e} kg", body_state.mass));
}
