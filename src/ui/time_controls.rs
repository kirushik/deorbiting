//! Time controls panel at the bottom of the screen.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asteroid::ResetEvent;
use crate::prediction::PredictionSettings;
use crate::types::{j2000_seconds_to_date_string, SimulationTime};

/// System that renders the time controls panel.
pub fn time_controls_panel(
    mut contexts: EguiContexts,
    mut sim_time: ResMut<SimulationTime>,
    mut reset_events: EventWriter<ResetEvent>,
    mut prediction_settings: ResMut<PredictionSettings>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    egui::TopBottomPanel::bottom("time_controls")
        .frame(
            egui::Frame::none()
                .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 30, 220))
                .inner_margin(egui::Margin::symmetric(16.0, 8.0)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Play/Pause button
                let icon = if sim_time.paused { "\u{25B6}" } else { "\u{23F8}" };
                if ui
                    .button(icon)
                    .on_hover_text(if sim_time.paused {
                        "Play (Space)"
                    } else {
                        "Pause (Space)"
                    })
                    .clicked()
                {
                    sim_time.paused = !sim_time.paused;
                }

                ui.separator();

                // Date/time display
                ui.label(
                    egui::RichText::new(j2000_seconds_to_date_string(sim_time.current))
                        .monospace(),
                );

                ui.separator();

                // Time scale buttons (mutually exclusive)
                ui.label("Speed:");
                for (i, scale) in [1.0, 10.0, 100.0, 1000.0].iter().enumerate() {
                    let label = format!("{}x", *scale as i32);
                    let is_selected = (sim_time.scale - scale).abs() < 0.01;
                    if ui
                        .selectable_label(is_selected, label)
                        .on_hover_text(format!("Set time scale ({})", i + 1))
                        .clicked()
                    {
                        sim_time.scale = *scale;
                    }
                }

                ui.separator();

                // Prediction horizon slider
                ui.label("Prediction:");
                let years = (prediction_settings.max_time / (365.25 * 24.0 * 3600.0)) as f32;
                let mut years_slider = years;
                if ui
                    .add(
                        egui::Slider::new(&mut years_slider, 1.0..=10.0)
                            .suffix(" yr")
                            .fixed_decimals(0),
                    )
                    .changed()
                {
                    prediction_settings.max_time =
                        years_slider as f64 * 365.25 * 24.0 * 3600.0;
                    prediction_settings.max_steps = (years_slider as usize) * 10_000;
                }

                ui.separator();

                // Reset button
                if ui
                    .button("\u{21BA}")
                    .on_hover_text("Reset simulation (R)")
                    .clicked()
                {
                    reset_events.send(ResetEvent);
                }
            });
        });
}
