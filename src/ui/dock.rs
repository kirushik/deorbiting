//! Unified dock (bottom bar) for all primary controls.
//!
//! The dock provides a single horizontal strip with:
//! - Play/Pause toggle
//! - Human-readable date
//! - Speed dots (1x, 10x, 100x, 1000x)
//! - Current scenario name
//! - Reset button
//! - Scenarios drawer toggle
//! - Help button with shortcuts tooltip

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asteroid::{Asteroid, AsteroidName, ResetEvent};
use crate::render::SelectedBody;
use crate::scenarios::{get_scenario, CurrentScenario};
use crate::types::{SelectableBody, SimulationTime};

/// Resource for asteroid list popup state.
#[derive(Resource, Default)]
pub struct AsteroidListState {
    pub open: bool,
}

use super::ScenarioDrawerState;

/// Colors for the dock UI.
mod colors {
    use bevy_egui::egui::Color32;

    pub const DOCK_BG: Color32 = Color32::from_rgba_premultiplied(26, 26, 36, 240);
    pub const SPEED_ACTIVE: Color32 = Color32::from_rgb(85, 221, 136);
    pub const SPEED_INACTIVE: Color32 = Color32::from_rgb(120, 120, 130);
    pub const PLAY_ICON: Color32 = Color32::from_rgb(85, 221, 136);
    pub const PAUSE_ICON: Color32 = Color32::from_rgb(221, 170, 85);
    pub const TEXT: Color32 = Color32::from_rgb(220, 220, 230);
}

/// Resource for help tooltip visibility.
#[derive(Resource, Default)]
pub struct HelpTooltipState {
    pub visible: bool,
}

/// System that renders the unified dock at the bottom.
#[allow(clippy::too_many_arguments)]
pub fn dock_system(
    mut contexts: EguiContexts,
    mut sim_time: ResMut<SimulationTime>,
    mut reset_events: EventWriter<ResetEvent>,
    current_scenario: Res<CurrentScenario>,
    mut drawer_state: ResMut<ScenarioDrawerState>,
    mut help_state: ResMut<HelpTooltipState>,
    asteroids: Query<(Entity, &AsteroidName), With<Asteroid>>,
    mut selected: ResMut<SelectedBody>,
    mut placement_mode: ResMut<super::AsteroidPlacementMode>,
    mut asteroid_list_state: ResMut<AsteroidListState>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    let dock_height = 56.0;

    egui::TopBottomPanel::bottom("dock")
        .exact_height(dock_height)
        .frame(
            egui::Frame::none()
                .fill(colors::DOCK_BG)
                .inner_margin(egui::Margin::symmetric(20.0, 10.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 16.0;

                // Play/Pause button
                render_play_pause(ui, &mut sim_time);

                ui.separator();

                // Date display
                render_date_display(ui, sim_time.current);

                ui.separator();

                // Speed dots
                render_speed_dots(ui, &mut sim_time);

                ui.separator();

                // Scenario name (clickable)
                render_scenario_name(ui, &current_scenario, &mut drawer_state);

                ui.separator();

                // Asteroid count indicator + spawn button
                render_asteroid_count(
                    ui,
                    ctx,
                    &asteroids,
                    &mut selected,
                    &mut placement_mode,
                    &mut asteroid_list_state,
                );

                // Spacer to push remaining buttons to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;

                    // Help button (rightmost)
                    render_help_button(ui, &mut help_state);

                    // Scenarios button
                    render_scenarios_button(ui, &mut drawer_state);

                    // Reset button
                    render_reset_button(ui, &mut reset_events);
                });
            });
        });

    // Render help tooltip if visible
    if help_state.visible {
        render_help_overlay(ctx);
    }
}

/// Render the play/pause toggle button.
fn render_play_pause(ui: &mut egui::Ui, sim_time: &mut SimulationTime) {
    use crate::ui::icons;

    let (icon, color, tooltip) = if sim_time.paused {
        (icons::PLAY, colors::PLAY_ICON, "Play (Space)")
    } else {
        (icons::PAUSE, colors::PAUSE_ICON, "Pause (Space)")
    };

    let button = egui::Button::new(egui::RichText::new(icon).size(22.0).color(color))
        .min_size(egui::vec2(40.0, 36.0));

    if ui.add(button).on_hover_text(tooltip).clicked() {
        sim_time.paused = !sim_time.paused;
    }
}

/// Render the date display in human-readable format.
fn render_date_display(ui: &mut egui::Ui, j2000_seconds: f64) {
    let date_str = format_date_human(j2000_seconds);
    ui.label(egui::RichText::new(date_str).monospace().size(14.0).color(colors::TEXT));
}

/// Render speed dots (4 dots for 1x, 10x, 100x, 1000x).
fn render_speed_dots(ui: &mut egui::Ui, sim_time: &mut SimulationTime) {
    let speeds = [1.0, 10.0, 100.0, 1000.0];
    let labels = ["1x", "10x", "100x", "1000x"];
    let current_index = speeds.iter().position(|&s| (sim_time.scale - s).abs() < 0.01);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (i, (&speed, label)) in speeds.iter().zip(labels.iter()).enumerate() {
            let is_active = current_index == Some(i);
            let color = if is_active { colors::SPEED_ACTIVE } else { colors::SPEED_INACTIVE };

            // Consistent 14px size, only weight differs
            let text = if is_active {
                egui::RichText::new(*label).size(14.0).color(color).strong()
            } else {
                egui::RichText::new(*label).size(14.0).color(color)
            };

            let tooltip = format!("{}x speed (press {})", speed as i32, i + 1);

            if ui.add(
                egui::Button::new(text)
                    .frame(is_active)
                    .min_size(egui::vec2(40.0, 28.0))
            ).on_hover_text(tooltip).clicked() {
                sim_time.scale = speed;
            }
        }
    });
}

/// Render the current scenario name (clickable to open drawer).
fn render_scenario_name(
    ui: &mut egui::Ui,
    current_scenario: &CurrentScenario,
    drawer_state: &mut ScenarioDrawerState,
) {
    let scenario_name = get_scenario(current_scenario.id)
        .map(|s| s.name)
        .unwrap_or("Unknown Scenario");

    let button = egui::Button::new(
        egui::RichText::new(scenario_name).size(14.0).color(colors::TEXT)
    ).frame(false);

    if ui.add(button).on_hover_text("Click to change scenario").clicked() {
        drawer_state.open = !drawer_state.open;
    }
}

/// Render asteroid count indicator with interactive list and spawn button.
fn render_asteroid_count(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    asteroids: &Query<(Entity, &AsteroidName), With<Asteroid>>,
    selected: &mut ResMut<SelectedBody>,
    placement_mode: &mut super::AsteroidPlacementMode,
    list_state: &mut AsteroidListState,
) {
    use crate::ui::icons;

    let asteroid_list: Vec<_> = asteroids.iter().collect();
    let count = asteroid_list.len();
    let has_selection = matches!(selected.body, Some(SelectableBody::Asteroid(_)));

    // Determine which asteroid is selected (if any)
    let selected_entity = match selected.body {
        Some(SelectableBody::Asteroid(e)) => Some(e),
        _ => None,
    };

    // Interactive asteroid button - shows count and opens list
    let text = if count == 0 {
        format!("{} No asteroids", icons::ASTEROID)
    } else {
        let icon = if list_state.open { icons::COLLAPSE } else { icons::EXPAND };
        format!("{} {} asteroid{}", icon, count, if count == 1 { "" } else { "s" })
    };

    let color = if has_selection {
        colors::SPEED_ACTIVE
    } else {
        colors::TEXT
    };

    let button = egui::Button::new(
        egui::RichText::new(text).size(14.0).color(color)
    ).frame(false);

    let response = ui.add(button);
    if count > 0 && response.clicked() {
        list_state.open = !list_state.open;
    }
    if count > 0 {
        response.on_hover_text("Click to show asteroid list");
    }

    // Show popup list of asteroids
    if list_state.open && count > 0 {
        egui::Window::new("Asteroids")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(200.0, -70.0))
            .frame(
                egui::Frame::none()
                    .fill(colors::DOCK_BG)
                    .inner_margin(8.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 80)))
                    .rounding(4.0),
            )
            .show(ctx, |ui| {
                ui.set_max_width(180.0);

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Asteroids").strong().size(13.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(icons::CLOSE).clicked() {
                            list_state.open = false;
                        }
                    });
                });
                ui.separator();

                for (entity, name) in asteroid_list.iter() {
                    let is_selected = selected_entity == Some(*entity);

                    let item_text = if is_selected {
                        egui::RichText::new(&name.0).strong().color(colors::SPEED_ACTIVE)
                    } else {
                        egui::RichText::new(&name.0).color(colors::TEXT)
                    };

                    let item_button = egui::Button::new(item_text.size(13.0))
                        .frame(is_selected)
                        .min_size(egui::vec2(160.0, 24.0));

                    if ui.add(item_button).clicked() {
                        selected.body = Some(SelectableBody::Asteroid(*entity));
                        list_state.open = false;
                    }
                }
            });
    }

    // Close popup when clicking outside
    if list_state.open {
        if ctx.input(|i| i.pointer.any_pressed()) {
            // We'll close in the next frame if click was outside the popup
            // (egui handles this via the Window's behavior)
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            list_state.open = false;
        }
    }

    // Spawn asteroid button
    let button_color = if placement_mode.active {
        colors::SPEED_ACTIVE
    } else {
        colors::SPEED_INACTIVE
    };

    let tooltip = if placement_mode.active {
        "Click on viewport to place asteroid (Right-click to cancel)"
    } else {
        "Add new asteroid"
    };

    let add_button = egui::Button::new(
        egui::RichText::new(icons::ADD).size(16.0).color(button_color)
    ).min_size(egui::vec2(28.0, 28.0));

    if ui.add(add_button).on_hover_text(tooltip).clicked() {
        placement_mode.active = !placement_mode.active;
    }
}

/// Render the reset button.
fn render_reset_button(ui: &mut egui::Ui, reset_events: &mut EventWriter<ResetEvent>) {
    use crate::ui::icons;

    let button = egui::Button::new(egui::RichText::new(icons::RESET).size(18.0))
        .min_size(egui::vec2(36.0, 32.0));

    if ui.add(button).on_hover_text("Reset scenario (R)").clicked() {
        reset_events.send(ResetEvent);
    }
}

/// Render the scenarios drawer toggle button.
fn render_scenarios_button(ui: &mut egui::Ui, drawer_state: &mut ScenarioDrawerState) {
    use crate::ui::icons;

    let icon = if drawer_state.open { icons::COLLAPSE } else { icons::MENU };
    let button = egui::Button::new(egui::RichText::new(icon).size(18.0))
        .min_size(egui::vec2(32.0, 32.0));

    if ui.add(button).on_hover_text("Scenarios (Esc)").clicked() {
        drawer_state.open = !drawer_state.open;
    }
}

/// Render the help button.
fn render_help_button(ui: &mut egui::Ui, help_state: &mut HelpTooltipState) {
    use crate::ui::icons;

    let button = egui::Button::new(egui::RichText::new(icons::HELP).size(18.0))
        .min_size(egui::vec2(32.0, 32.0));

    let response = ui.add(button);

    if response.hovered() || help_state.visible {
        help_state.visible = response.hovered();
    }

    if response.clicked() {
        help_state.visible = !help_state.visible;
    }
}

/// Render the help overlay showing keyboard shortcuts.
fn render_help_overlay(ctx: &egui::Context) {
    egui::Window::new("Keyboard Shortcuts")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-70.0, -70.0))
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_premultiplied(26, 26, 36, 245))
                .inner_margin(16.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 80))),
        )
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;

            let shortcuts = [
                ("Space", "Play/Pause"),
                ("1-4", "Set speed (1x/10x/100x/1000x)"),
                ("R", "Reset scenario"),
                ("Esc", "Open scenarios"),
                ("Del", "Delete selected asteroid"),
                ("+/-", "Zoom in/out"),
            ];

            egui::Grid::new("shortcuts_grid")
                .num_columns(2)
                .spacing([20.0, 6.0])
                .show(ui, |ui| {
                    for (key, action) in shortcuts {
                        ui.label(egui::RichText::new(key).strong().monospace().size(14.0));
                        ui.label(egui::RichText::new(action).size(14.0));
                        ui.end_row();
                    }
                });
        });
}

/// Format J2000 seconds as a human-readable date (e.g., "Jan 15, 2026").
fn format_date_human(j2000_seconds: f64) -> String {
    const J2000_UNIX: f64 = 946_728_000.0;
    let unix_secs = J2000_UNIX + j2000_seconds;
    let days_since_epoch = (unix_secs / 86400.0).floor() as i64;
    let (year, month, day) = days_to_ymd(days_since_epoch);

    let month_names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let month_name = month_names.get((month - 1) as usize).unwrap_or(&"???");
    format!("{} {}, {}", month_name, day, year)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    (year as i32, m, d)
}
