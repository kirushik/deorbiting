//! Mission status panel for active continuous deflectors.
//!
//! Displays information about ongoing continuous deflection missions:
//! - Method type and state
//! - Fuel remaining (for ion beam)
//! - Accumulated delta-v
//! - Progress indicators

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::continuous::{ContinuousDeflector, ContinuousDeflectorState, ContinuousPayload};

/// System to render the mission status panel for active deflectors.
pub fn mission_status_panel(
    mut contexts: EguiContexts,
    deflectors: Query<(Entity, &ContinuousDeflector)>,
) {
    // Count active deflectors
    let active_deflectors: Vec<_> = deflectors
        .iter()
        .filter(|(_, d)| d.is_operating() || matches!(d.state, ContinuousDeflectorState::EnRoute { .. }))
        .collect();

    if active_deflectors.is_empty() {
        return;
    }

    let ctx = contexts.ctx_mut();

    egui::Window::new("🛰️ Active Missions")
        .collapsible(true)
        .resizable(false)
        .anchor(egui::Align2::LEFT_CENTER, egui::vec2(20.0, 0.0))
        .default_width(250.0)
        .show(ctx, |ui| {
            for (i, (_, deflector)) in active_deflectors.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                render_deflector_status(ui, deflector);
            }
        });
}

/// Render status for a single deflector.
fn render_deflector_status(ui: &mut egui::Ui, deflector: &ContinuousDeflector) {
    let method_name = deflector.payload.name();
    let (color, icon) = method_color_icon(&deflector.payload);

    // Header with method type and icon
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).color(color));
        ui.label(egui::RichText::new(method_name).strong());
    });

    // State indicator
    match &deflector.state {
        ContinuousDeflectorState::EnRoute { arrival_time: _ } => {
            ui.label(egui::RichText::new("⏳ En route").color(egui::Color32::YELLOW));
        }
        ContinuousDeflectorState::Operating {
            fuel_consumed,
            accumulated_delta_v,
            ..
        } => {
            ui.label(egui::RichText::new("✓ Operating").color(egui::Color32::GREEN));

            // Show accumulated delta-v
            ui.label(format!("Δv applied: {:.4} mm/s", accumulated_delta_v * 1000.0));

            // Show fuel remaining for ion beam
            if let Some(initial_fuel) = deflector.payload.initial_fuel() {
                let remaining = (initial_fuel - fuel_consumed).max(0.0);
                let fraction = remaining / initial_fuel;

                ui.horizontal(|ui| {
                    ui.label("Fuel:");
                    let progress_bar = egui::ProgressBar::new(fraction as f32)
                        .show_percentage()
                        .fill(if fraction > 0.2 {
                            egui::Color32::from_rgb(100, 200, 100)
                        } else {
                            egui::Color32::from_rgb(255, 150, 50)
                        });
                    ui.add(progress_bar);
                });
            }
        }
        ContinuousDeflectorState::FuelDepleted { total_delta_v, .. } => {
            ui.label(egui::RichText::new("⛽ Fuel depleted").color(egui::Color32::from_rgb(255, 150, 50)));
            ui.label(format!("Total Δv: {:.4} mm/s", total_delta_v * 1000.0));
        }
        ContinuousDeflectorState::Complete { total_delta_v, .. } => {
            ui.label(egui::RichText::new("✓ Complete").color(egui::Color32::GREEN));
            ui.label(format!("Total Δv: {:.4} mm/s", total_delta_v * 1000.0));
        }
        ContinuousDeflectorState::Cancelled => {
            ui.label(egui::RichText::new("✗ Cancelled").color(egui::Color32::RED));
        }
    }
}

/// Get color and icon for a deflection method.
fn method_color_icon(payload: &ContinuousPayload) -> (egui::Color32, &'static str) {
    match payload {
        ContinuousPayload::IonBeam { .. } => {
            (egui::Color32::from_rgb(0, 200, 255), "⚡") // Cyan
        }
        ContinuousPayload::GravityTractor { .. } => {
            (egui::Color32::from_rgb(200, 100, 255), "🌀") // Purple
        }
        ContinuousPayload::LaserAblation { .. } => {
            (egui::Color32::from_rgb(255, 150, 50), "🔆") // Orange
        }
        ContinuousPayload::SolarSail { .. } => {
            (egui::Color32::from_rgb(255, 230, 80), "⛵") // Yellow/Gold
        }
    }
}
