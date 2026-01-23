//! Radial deflection menu - in-place tool selector.
//!
//! When "Deflect" is clicked on a context card, a radial menu appears
//! around the asteroid showing all deflection method options.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::asteroid::Asteroid;
use crate::continuous::LaunchContinuousDeflectorEvent;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::interceptor::LaunchInterceptorEvent;
use crate::types::{AU_TO_METERS, BodyState, SECONDS_PER_DAY, SimulationTime};

use super::deflection_helpers::{
    ALL_METHODS, DeflectionMethod, apply_deflection, calculate_flight_time_from_earth,
};

/// Resource for radial menu state.
#[derive(Resource, Default)]
pub struct RadialMenuState {
    /// Whether the menu is open.
    pub open: bool,
    /// Target asteroid entity.
    pub target: Option<Entity>,
    /// Screen position of the menu center.
    pub position: Vec2,
}

/// Colors for the radial menu.
mod colors {
    use bevy_egui::egui::Color32;

    pub const MENU_BG: Color32 = Color32::from_rgba_premultiplied(26, 26, 36, 230);
    pub const BUTTON_BG: Color32 = Color32::from_rgba_premultiplied(50, 50, 70, 255);
    pub const BUTTON_HOVER: Color32 = Color32::from_rgba_premultiplied(70, 70, 100, 255);
}

/// System to render the radial deflection menu.
/// The menu is opened via right-click on asteroid (handled by selection system).
#[allow(clippy::too_many_arguments)]
pub fn radial_menu_system(
    mut contexts: EguiContexts,
    mut menu_state: ResMut<RadialMenuState>,
    mut launch_events: MessageWriter<LaunchInterceptorEvent>,
    mut continuous_launch_events: MessageWriter<LaunchContinuousDeflectorEvent>,
    asteroids: Query<&BodyState, With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
) {
    if !menu_state.open {
        return;
    }

    let Some(target) = menu_state.target else {
        menu_state.open = false;
        return;
    };

    let Ok(asteroid_state) = asteroids.get(target) else {
        menu_state.open = false;
        return;
    };

    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    // Calculate distance to Earth for flight time
    let earth_pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
        .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
    let flight_time_seconds = calculate_flight_time_from_earth(asteroid_state.pos, earth_pos);
    let flight_time_days = flight_time_seconds / SECONDS_PER_DAY;

    let center = egui::pos2(menu_state.position.x, menu_state.position.y);
    let radius = 90.0;

    // Draw the radial menu (non-modal - allows viewport interaction)
    egui::Area::new(egui::Id::new("radial_menu"))
        .fixed_pos(center - egui::vec2(radius + 50.0, radius + 50.0))
        .interactable(true)
        .show(ctx, |ui| {
            // Background circle
            ui.painter().circle(
                center,
                radius + 40.0,
                colors::MENU_BG,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 60, 80)),
            );

            // Flight time indicator at center - use primary color for mission-critical info
            ui.painter().text(
                center - egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}d", flight_time_days),
                egui::FontId::proportional(16.0),
                egui::Color32::from_rgb(220, 220, 230), // PRIMARY - mission-critical
            );
            ui.painter().text(
                center + egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                "flight",
                egui::FontId::proportional(13.0), // Minimum readable size
                egui::Color32::from_rgb(220, 220, 230), // PRIMARY
            );

            // Arrange all 7 methods in a circle
            let num_methods = ALL_METHODS.len();
            for (i, method) in ALL_METHODS.iter().enumerate() {
                // Start from top and go clockwise
                let angle = -std::f32::consts::FRAC_PI_2
                    + (i as f32 * std::f32::consts::TAU / num_methods as f32);
                let button_pos = center + egui::vec2(angle.cos() * radius, angle.sin() * radius);

                // No keyboard shortcut display (removed to avoid conflict with dock speed keys)
                if render_method_button(ui, button_pos, *method) {
                    apply_deflection(
                        target,
                        *method,
                        asteroid_state,
                        flight_time_seconds,
                        &mut launch_events,
                        &mut continuous_launch_events,
                    );
                    menu_state.open = false;
                }
            }
        });

    // Close on click outside the menu area
    if ctx.input(|i| i.pointer.any_pressed())
        && let Some(pos) = ctx.input(|i| i.pointer.hover_pos())
    {
        let dist = ((pos.x - center.x).powi(2) + (pos.y - center.y).powi(2)).sqrt();
        if dist > radius + 60.0 {
            menu_state.open = false;
        }
    }

    // Close on Escape
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        menu_state.open = false;
    }

    // NOTE: Keyboard shortcuts 1-7 removed to avoid conflict with dock speed keys.
    // Use inline deflection strip in context card or banner for quick access.
}

/// Render a method button. Returns true if clicked.
fn render_method_button(ui: &mut egui::Ui, pos: egui::Pos2, method: DeflectionMethod) -> bool {
    let button_size = 44.0;
    let rect = egui::Rect::from_center_size(pos, egui::vec2(button_size, button_size));

    let response = ui.allocate_rect(rect, egui::Sense::click());

    let bg_color = if response.hovered() {
        colors::BUTTON_HOVER
    } else {
        colors::BUTTON_BG
    };

    let accent_color = method.color();

    // Draw button
    ui.painter().rect(
        rect,
        6.0,
        bg_color,
        egui::Stroke::new(2.0, accent_color),
        egui::StrokeKind::Middle,
    );

    // Icon (must use explicit Phosphor font family)
    ui.painter().text(
        pos,
        egui::Align2::CENTER_CENTER,
        method.icon(),
        egui::FontId::new(20.0, egui::FontFamily::Name("phosphor".into())),
        egui::Color32::WHITE,
    );

    // Label below
    ui.painter().text(
        pos + egui::vec2(0.0, button_size / 2.0 + 10.0),
        egui::Align2::CENTER_TOP,
        method.name(),
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(200, 200, 210),
    );

    // Type indicator (instant vs continuous)
    let type_label = if method.is_continuous() {
        "cont."
    } else {
        "inst."
    };
    ui.painter().text(
        pos + egui::vec2(0.0, button_size / 2.0 + 22.0),
        egui::Align2::CENTER_TOP,
        type_label,
        egui::FontId::proportional(11.0), // Truly supplementary - OK at 11px
        egui::Color32::from_rgb(180, 180, 190), // Secondary text
    );

    response.clicked()
}
