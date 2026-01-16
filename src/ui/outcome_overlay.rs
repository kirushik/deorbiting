//! Trajectory outcome overlay display.
//!
//! Shows visual feedback when a trajectory outcome is determined:
//! - Collision (red): "IMPACT!" with body and velocity
//! - Escape (blue): "ESCAPE!" with v_infinity
//! - Stable orbit (green): "STABLE ORBIT!" with orbital parameters

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asteroid::ResetEvent;
use crate::outcome::TrajectoryOutcome;
use crate::prediction::TrajectoryPath;
use crate::render::SelectedBody;
use crate::scenarios::{CurrentScenario, ScenarioMenuState};
use crate::types::{SelectableBody, AU_TO_METERS, SECONDS_PER_DAY};

/// Resource for outcome overlay state.
#[derive(Resource, Default)]
pub struct OutcomeOverlayState {
    /// Currently displayed outcome (if any).
    pub displayed: Option<TrajectoryOutcome>,
    /// Whether to show the overlay.
    pub visible: bool,
    /// Flash effect progress (0.0 to 1.0, used for collision).
    pub flash_progress: f32,
}

/// System to update outcome overlay state from selected asteroid.
pub fn update_outcome_state(
    selected: Res<SelectedBody>,
    trajectories: Query<&TrajectoryPath>,
    mut overlay_state: ResMut<OutcomeOverlayState>,
) {
    // Get selected asteroid's trajectory
    let outcome = if let Some(SelectableBody::Asteroid(entity)) = selected.body {
        if let Ok(trajectory) = trajectories.get(entity) {
            trajectory.outcome.clone()
        } else {
            TrajectoryOutcome::InProgress
        }
    } else {
        TrajectoryOutcome::InProgress
    };

    // Update state
    let new_outcome = if outcome.is_determined() {
        Some(outcome.clone())
    } else {
        None
    };

    // Check if outcome changed
    let outcome_changed = match (&overlay_state.displayed, &new_outcome) {
        (None, None) => false,
        (Some(_), None) | (None, Some(_)) => true,
        (Some(old), Some(new)) => !matches!(
            (old, new),
            (TrajectoryOutcome::Collision { .. }, TrajectoryOutcome::Collision { .. })
                | (TrajectoryOutcome::Escape { .. }, TrajectoryOutcome::Escape { .. })
                | (TrajectoryOutcome::StableOrbit { .. }, TrajectoryOutcome::StableOrbit { .. })
        ),
    };

    if outcome_changed {
        overlay_state.displayed = new_outcome;
        overlay_state.visible = overlay_state.displayed.is_some();

        // Start flash for collision
        if matches!(overlay_state.displayed, Some(TrajectoryOutcome::Collision { .. })) {
            overlay_state.flash_progress = 1.0;
        }
    }
}

/// System to animate the flash effect.
pub fn animate_flash(time: Res<Time>, mut overlay_state: ResMut<OutcomeOverlayState>) {
    if overlay_state.flash_progress > 0.0 {
        // Fade over 0.3 seconds
        overlay_state.flash_progress -= time.delta_secs() / 0.3;
        overlay_state.flash_progress = overlay_state.flash_progress.max(0.0);
    }
}

/// System to render the outcome overlay.
pub fn outcome_overlay_system(
    mut contexts: EguiContexts,
    overlay_state: Res<OutcomeOverlayState>,
    current_scenario: Res<CurrentScenario>,
    mut reset_events: EventWriter<ResetEvent>,
    mut menu_state: ResMut<ScenarioMenuState>,
) {
    if !overlay_state.visible {
        return;
    }

    let Some(outcome) = &overlay_state.displayed else {
        return;
    };

    let ctx = contexts.ctx_mut();

    // Draw flash effect for collision
    if overlay_state.flash_progress > 0.0 {
        if let TrajectoryOutcome::Collision { .. } = outcome {
            let alpha = (overlay_state.flash_progress * 0.3) as u8;
            egui::Area::new(egui::Id::new("impact_flash"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let screen_rect = ui.ctx().screen_rect();
                    ui.painter().rect_filled(
                        screen_rect,
                        0.0,
                        egui::Color32::from_rgba_unmultiplied(255, 0, 0, alpha * 255 / 100),
                    );
                });
        }
    }

    // Determine colors and content based on outcome type
    let (title, color, content) = match outcome {
        TrajectoryOutcome::Collision {
            body_hit,
            time_to_impact,
            impact_velocity,
        } => {
            let days = time_to_impact / SECONDS_PER_DAY;
            let speed_km_s = impact_velocity / 1000.0;
            (
                "⚠️ COLLISION PREDICTED",
                egui::Color32::from_rgb(220, 50, 50),
                format!(
                    "Target: {body_hit:?}\nTime to impact: {days:.1} days\nImpact velocity: {speed_km_s:.1} km/s"
                ),
            )
        }

        TrajectoryOutcome::Escape { v_infinity, direction } => {
            let v_km_s = v_infinity / 1000.0;
            let angle = direction.y.atan2(direction.x).to_degrees();
            (
                "🚀 ESCAPE TRAJECTORY",
                egui::Color32::from_rgb(50, 150, 220),
                format!("V∞: {v_km_s:.2} km/s\nDirection: {angle:.0}°"),
            )
        }

        TrajectoryOutcome::StableOrbit {
            semi_major_axis,
            eccentricity,
            period,
            perihelion,
            aphelion,
        } => {
            let a_au = semi_major_axis / AU_TO_METERS;
            let period_days = period / SECONDS_PER_DAY;
            let period_years = period_days / 365.25;
            let peri_au = perihelion / AU_TO_METERS;
            let apo_au = aphelion / AU_TO_METERS;
            (
                "✓ STABLE ORBIT",
                egui::Color32::from_rgb(50, 200, 100),
                format!(
                    "Semi-major axis: {a_au:.3} AU\n\
                     Eccentricity: {eccentricity:.4}\n\
                     Period: {period_years:.2} years ({period_days:.0} days)\n\
                     Perihelion: {peri_au:.3} AU\n\
                     Aphelion: {apo_au:.3} AU"
                ),
            )
        }

        TrajectoryOutcome::InProgress => return, // Shouldn't happen, but handle gracefully
    };

    // Draw overlay panel
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 50.0))
        .frame(egui::Frame::none().fill(color.gamma_multiply(0.2)).inner_margin(12.0))
        .show(ctx, |ui| {
            ui.visuals_mut().override_text_color = Some(egui::Color32::WHITE);

            // Title with colored background
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(title)
                        .size(18.0)
                        .color(color)
                        .strong(),
                );
            });

            ui.add_space(8.0);

            // Content
            ui.label(content);

            ui.add_space(8.0);

            // Hint text
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("(Updates automatically with trajectory)").weak().small());
            });

            // Reset/New Scenario buttons for collision outcomes
            if matches!(outcome, TrajectoryOutcome::Collision { .. }) {
                ui.add_space(8.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Reset Scenario").clicked() {
                        reset_events.send(ResetEvent);
                    }
                    if ui.button("New Scenario").clicked() {
                        menu_state.open = true;
                    }
                });
            }

            // Congratulations message for Deflection Challenge scenario
            if matches!(outcome, TrajectoryOutcome::StableOrbit { .. })
                && current_scenario.id == "deflection_challenge"
            {
                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("🎉 Congratulations! You saved Earth!")
                        .color(egui::Color32::from_rgb(50, 200, 100))
                        .size(16.0),
                );
                ui.label("The asteroid will safely orbit the Sun.");
            }
        });
}
