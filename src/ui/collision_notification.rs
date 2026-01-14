//! Collision notification overlay.
//!
//! Shows notifications when asteroids collide with celestial bodies.
//! Uses a queue-based system to handle multiple collisions properly:
//! - Each collision gets its own notification
//! - Notifications are shown in order (FIFO)
//! - Dismissing advances to the next notification
//! - Resuming simulation clears the current notification

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::collision::{CollisionEvent, CollisionState};
use crate::types::SimulationTime;

/// Resource tracking the currently displayed collision notification.
///
/// This is separate from CollisionState to clearly distinguish between
/// "pending notifications" (in queue) and "currently displayed" (here).
#[derive(Resource, Default)]
pub struct ActiveNotification {
    /// The collision event currently being displayed, if any.
    pub current: Option<CollisionEvent>,
}

/// System that renders collision notifications.
///
/// Pulls notifications from the CollisionState queue and displays them
/// one at a time. Notifications are cleared when:
/// - The user clicks "Dismiss" (advances to next notification)
/// - The simulation is resumed (clears current notification)
pub fn collision_notification(
    mut contexts: EguiContexts,
    mut collision_state: ResMut<CollisionState>,
    mut active: ResMut<ActiveNotification>,
    sim_time: Res<SimulationTime>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    // If simulation resumed, clear current notification
    if !sim_time.paused {
        active.current = None;
    }

    // If no current notification and we're paused, try to pop one from queue
    if active.current.is_none() && sim_time.paused {
        active.current = collision_state.pop_notification();
    }

    // Nothing to display
    let Some(collision) = active.current.clone() else {
        return;
    };

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
                // Impact icon
                ui.label(egui::RichText::new("\u{1F4A5}").size(32.0));

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

                // Show if more notifications are pending
                if collision_state.has_pending() {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "({} more collision{})",
                            collision_state.pending_notifications.len(),
                            if collision_state.pending_notifications.len() == 1 { "" } else { "s" }
                        ))
                        .weak()
                        .small(),
                    );
                }

                ui.add_space(16.0);

                // Instructions
                ui.label(
                    egui::RichText::new("Press Play to continue simulation")
                        .weak()
                        .italics(),
                );

                ui.add_space(8.0);

                // Dismiss button - clears current notification, next frame will pop another if any
                if ui.button("Dismiss").clicked() {
                    active.current = None;
                }
            });
        });
}
