//! Scenario selection menu.
//!
//! Modal window for selecting and loading predefined scenarios.
//! Opens with Escape or M key.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::scenarios::{LoadScenarioEvent, ScenarioMenuState, SCENARIOS};
use crate::types::SimulationTime;

/// System to render the scenario selection menu.
pub fn scenario_menu_system(
    mut contexts: EguiContexts,
    mut menu_state: ResMut<ScenarioMenuState>,
    mut sim_time: ResMut<SimulationTime>,
    mut load_events: EventWriter<LoadScenarioEvent>,
) {
    if !menu_state.open {
        return;
    }

    // Pause simulation while menu is open
    let was_paused = sim_time.paused;
    sim_time.paused = true;

    let ctx = contexts.ctx_mut();

    // Center the window
    egui::Window::new("🚀 Scenario Selection")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;

            ui.label("Select a scenario to explore orbital mechanics concepts:");
            ui.add_space(8.0);

            // Scenario list with radio buttons
            egui::ScrollArea::vertical()
                .max_height(350.0)
                .show(ui, |ui| {
                    for (idx, scenario) in SCENARIOS.iter().enumerate() {
                        let is_selected = menu_state.selected_index == idx;

                        ui.horizontal(|ui| {
                            if ui.radio(is_selected, "").clicked() {
                                menu_state.selected_index = idx;
                            }

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.strong(scenario.name);
                                    if scenario.start_paused {
                                        ui.label("⏸");
                                    }
                                });
                                ui.label(scenario.description);

                                // Show additional info
                                ui.horizontal(|ui| {
                                    ui.label(format!("Time scale: {}x", scenario.time_scale));
                                    if scenario.start_paused {
                                        ui.label("• Starts paused");
                                    }
                                });
                            });
                        });

                        ui.add_space(4.0);
                        ui.separator();
                    }
                });

            ui.add_space(12.0);

            // Buttons
            ui.horizontal(|ui| {
                let load_button = ui.button("Load Scenario");
                if load_button.clicked() {
                    if let Some(scenario) = SCENARIOS.get(menu_state.selected_index) {
                        load_events.send(LoadScenarioEvent {
                            scenario_id: scenario.id,
                        });
                        menu_state.open = false;
                        // Restore pause state (loading might unpause)
                    }
                }

                if ui.button("Cancel").clicked() {
                    menu_state.open = false;
                    // Restore original pause state
                    sim_time.paused = was_paused;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("Press Escape or M to close");
                });
            });
        });
}

/// System to handle keyboard shortcuts for the scenario menu.
pub fn scenario_menu_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu_state: ResMut<ScenarioMenuState>,
) {
    // Toggle menu with Escape or M
    if keys.just_pressed(KeyCode::Escape) || keys.just_pressed(KeyCode::KeyM) {
        menu_state.open = !menu_state.open;
    }
}
