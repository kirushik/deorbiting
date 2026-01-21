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
use bevy_egui::{EguiContexts, egui};

use crate::asteroid::{
    Asteroid, AsteroidName, AsteroidVisual, ResetEvent, indicator_color_from_material,
};
use crate::render::SelectedBody;
use crate::scenarios::{CurrentScenario, get_scenario};
use crate::types::{SelectableBody, SimulationTime};

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
    mut reset_events: MessageWriter<ResetEvent>,
    current_scenario: Res<CurrentScenario>,
    mut drawer_state: ResMut<ScenarioDrawerState>,
    mut help_state: ResMut<HelpTooltipState>,
    asteroids: Query<(Entity, &AsteroidName, &AsteroidVisual), With<Asteroid>>,
    mut selected: ResMut<SelectedBody>,
    mut placement_mode: ResMut<super::AsteroidPlacementMode>,
    mut camera_focus_events: MessageWriter<crate::camera::FocusOnEntityEvent>,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    let dock_height = 56.0;

    egui::TopBottomPanel::bottom("dock")
        .exact_height(dock_height)
        .frame(
            egui::Frame::NONE
                .fill(colors::DOCK_BG)
                .inner_margin(egui::Margin::symmetric(20, 12)),
        )
        .show(ctx, |ui| {
            // Use a single horizontal_centered layout - NO nested horizontals!
            ui.horizontal_centered(|ui| {
                use crate::ui::icons;

                // ===== Play/Pause Button =====
                let (play_icon, play_color, play_tooltip) = if sim_time.paused {
                    (icons::PLAY, colors::PLAY_ICON, "Play (Space)")
                } else {
                    (icons::PAUSE, colors::PAUSE_ICON, "Pause (Space)")
                };
                if ui
                    .add_sized(
                        [DOCK_BUTTON_SIZE, DOCK_BUTTON_SIZE],
                        egui::Button::new(
                            egui::RichText::new(play_icon).size(18.0).color(play_color),
                        ),
                    )
                    .on_hover_text(play_tooltip)
                    .clicked()
                {
                    sim_time.paused = !sim_time.paused;
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // ===== Date Display (fixed width to prevent jumping) =====
                let date_str = format_date_human(sim_time.current);
                ui.add_sized(
                    [DATE_WIDTH, DOCK_BUTTON_SIZE],
                    egui::Label::new(
                        egui::RichText::new(date_str)
                            .monospace()
                            .size(14.0)
                            .color(colors::TEXT),
                    ),
                );

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // ===== Speed Buttons (inline, no nested horizontal) =====
                let speeds = [1.0, 10.0, 100.0, 1000.0];
                let labels = ["1x", "10x", "100x", "1000x"];
                let current_speed_index = speeds
                    .iter()
                    .position(|&s| (sim_time.scale - s).abs() < 0.01);

                for (i, (&speed, label)) in speeds.iter().zip(labels.iter()).enumerate() {
                    let is_active = current_speed_index == Some(i);
                    let color = if is_active {
                        colors::SPEED_ACTIVE
                    } else {
                        colors::SPEED_INACTIVE
                    };
                    let text = egui::RichText::new(*label).size(14.0).color(color).strong();

                    if ui
                        .add_sized(
                            [SPEED_BUTTON_WIDTH, DOCK_BUTTON_SIZE],
                            egui::Button::new(text).frame(false),
                        )
                        .on_hover_text(format!("{}x speed (press {})", speed as i32, i + 1))
                        .clicked()
                    {
                        sim_time.scale = speed;
                    }
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // ===== Scenario Name =====
                let scenario_name = get_scenario(current_scenario.id)
                    .map(|s| s.name)
                    .unwrap_or("Unknown Scenario");
                if ui
                    .add_sized(
                        [SCENARIO_NAME_WIDTH, DOCK_BUTTON_SIZE],
                        egui::Button::new(
                            egui::RichText::new(scenario_name)
                                .size(14.0)
                                .color(colors::TEXT),
                        )
                        .frame(false),
                    )
                    .on_hover_text("Click to change scenario")
                    .clicked()
                {
                    drawer_state.open = !drawer_state.open;
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(12.0);

                // ===== Asteroid Indicators (inline, no nested horizontal) =====
                let asteroid_list: Vec<_> = asteroids.iter().collect();
                let selected_entity = match selected.body {
                    Some(SelectableBody::Asteroid(e)) => Some(e),
                    _ => None,
                };

                if asteroid_list.is_empty() {
                    ui.add_sized(
                        [80.0, DOCK_BUTTON_SIZE],
                        egui::Label::new(
                            egui::RichText::new("No asteroids")
                                .size(13.0)
                                .color(colors::SPEED_INACTIVE),
                        ),
                    );
                } else {
                    for (entity, name, visual) in asteroid_list.iter() {
                        let is_selected = selected_entity == Some(*entity);

                        // Use asteroid's indicator color
                        let indicator = indicator_color_from_material(visual.color);
                        let rgba = indicator.to_srgba();
                        let dot_color = if is_selected {
                            // Full brightness when selected
                            egui::Color32::from_rgb(
                                (rgba.red * 255.0) as u8,
                                (rgba.green * 255.0) as u8,
                                (rgba.blue * 255.0) as u8,
                            )
                        } else {
                            // Slightly dimmed when not selected
                            egui::Color32::from_rgba_unmultiplied(
                                (rgba.red * 200.0) as u8,
                                (rgba.green * 200.0) as u8,
                                (rgba.blue * 200.0) as u8,
                                200,
                            )
                        };

                        let response = ui
                            .add_sized(
                                [DOCK_BUTTON_SIZE, DOCK_BUTTON_SIZE],
                                egui::Button::new(
                                    egui::RichText::new(icons::ASTEROID)
                                        .size(14.0)
                                        .color(dot_color),
                                )
                                .frame(is_selected),
                            )
                            .on_hover_text(&name.0);

                        if response.clicked() {
                            selected.body = Some(SelectableBody::Asteroid(*entity));
                        }
                        if response.double_clicked() {
                            // Double-click centers camera on asteroid
                            camera_focus_events
                                .write(crate::camera::FocusOnEntityEvent { entity: *entity });
                        }
                    }
                }

                // Add asteroid button
                let add_color = if placement_mode.active {
                    colors::SPEED_ACTIVE
                } else {
                    colors::SPEED_INACTIVE
                };
                let add_tooltip = if placement_mode.active {
                    "Click on viewport to place (Right-click to cancel)"
                } else {
                    "Add asteroid"
                };
                if ui
                    .add_sized(
                        [DOCK_BUTTON_SIZE, DOCK_BUTTON_SIZE],
                        egui::Button::new(
                            egui::RichText::new(icons::ADD).size(14.0).color(add_color),
                        ),
                    )
                    .on_hover_text(add_tooltip)
                    .clicked()
                {
                    placement_mode.active = !placement_mode.active;
                }

                // ===== Right-aligned buttons =====
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Help button
                    let help_response = ui.add_sized(
                        [DOCK_BUTTON_SIZE, DOCK_BUTTON_SIZE],
                        egui::Button::new(egui::RichText::new(icons::HELP).size(18.0)),
                    );
                    if help_response.hovered() || help_state.visible {
                        help_state.visible = help_response.hovered();
                    }
                    if help_response.clicked() {
                        help_state.visible = !help_state.visible;
                    }

                    // Scenarios button
                    let scenarios_icon = if drawer_state.open {
                        icons::COLLAPSE
                    } else {
                        icons::MENU
                    };
                    if ui
                        .add_sized(
                            [DOCK_BUTTON_SIZE, DOCK_BUTTON_SIZE],
                            egui::Button::new(egui::RichText::new(scenarios_icon).size(18.0)),
                        )
                        .on_hover_text("Scenarios (Esc)")
                        .clicked()
                    {
                        drawer_state.open = !drawer_state.open;
                    }

                    // Reset button
                    if ui
                        .add_sized(
                            [DOCK_BUTTON_SIZE, DOCK_BUTTON_SIZE],
                            egui::Button::new(egui::RichText::new(icons::RESET).size(18.0)),
                        )
                        .on_hover_text("Reset scenario (R)")
                        .clicked()
                    {
                        reset_events.write(ResetEvent);
                    }
                });
            });
        });

    // Render help tooltip if visible
    if help_state.visible {
        render_help_overlay(ctx);
    }
}

/// Standard button size for dock alignment (square buttons).
const DOCK_BUTTON_SIZE: f32 = 32.0;
/// Fixed width for date display to prevent layout jumping.
const DATE_WIDTH: f32 = 110.0;
/// Width for speed buttons.
const SPEED_BUTTON_WIDTH: f32 = 44.0;
/// Width for scenario name.
const SCENARIO_NAME_WIDTH: f32 = 160.0;

/// Render the help overlay showing keyboard shortcuts.
fn render_help_overlay(ctx: &egui::Context) {
    egui::Window::new("Keyboard Shortcuts")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-70.0, -70.0))
        .frame(
            egui::Frame::NONE
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
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let month_name = month_names.get((month - 1) as usize).unwrap_or(&"???");
    format!("{} {}, {}", month_name, day, year)
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 {
        z / 146097
    } else {
        (z - 146096) / 146097
    };
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
