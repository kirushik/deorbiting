//! Collision notification overlay.
//!
//! Shows a notification when an asteroid collides with a celestial body.
//! The notification auto-dismisses when the simulation is resumed.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::collision::CollisionState;
use crate::types::SimulationTime;

/// System that renders collision notifications.
pub fn collision_notification(
    mut contexts: EguiContexts,
    collision_state: Res<CollisionState>,
    sim_time: Res<SimulationTime>,
    mut dismissed: Local<bool>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    // Reset dismissed state when simulation resumes
    if !sim_time.paused {
        *dismissed = false;
    }

    // Don't show if dismissed or no collision
    if *dismissed {
        return;
    }

    let Some(collision) = &collision_state.last_collision else {
        return;
    };

    // Only show notification while paused (i.e., right after collision)
    if !sim_time.paused {
        return;
    }

    // Show notification as a centered window
    egui::Window::new("Impact Detected!")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, -50.0])
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgba_unmultiplied(40, 20, 20, 240))
                .stroke(egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 100))),
        )
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Impact icon and title
                ui.label(
                    egui::RichText::new("\u{1F4A5}")
                        .size(32.0),
                );

                ui.add_space(8.0);

                // Asteroid name
                ui.label(
                    egui::RichText::new(&collision.asteroid_name)
                        .strong()
                        .size(18.0),
                );

                ui.label(format!("collided with {}", collision.body_hit.name()));

                ui.add_space(12.0);

                // Impact details
                ui.label(format!(
                    "Impact velocity: {:.2} km/s",
                    collision.impact_speed_km_s()
                ));

                ui.add_space(16.0);

                // Instructions
                ui.label(
                    egui::RichText::new("Press Play to continue simulation")
                        .weak()
                        .italics(),
                );

                ui.add_space(8.0);

                // Dismiss button
                if ui.button("Dismiss").clicked() {
                    *dismissed = true;
                }
            });
        });
}
