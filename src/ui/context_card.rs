//! Context card - floating info panel near selected objects.
//!
//! Replaces the right info panel with a card that appears near the selected
//! object. Shows key information and actions inline.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asteroid::{Asteroid, AsteroidName};
use crate::camera::{MainCamera, RENDER_SCALE};
use crate::continuous::{ContinuousDeflector, ContinuousDeflectorState};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::render::{CelestialBody, SelectedBody};
use crate::types::{BodyState, SelectableBody, SimulationTime, AU_TO_METERS};

use super::RadialMenuState;

/// Colors for the context card.
mod colors {
    use bevy_egui::egui::Color32;

    pub const CARD_BG: Color32 = Color32::from_rgba_premultiplied(26, 26, 36, 230);
    pub const CARD_BORDER: Color32 = Color32::from_rgb(60, 60, 80);
    pub const DANGER: Color32 = Color32::from_rgb(224, 85, 85);
    pub const SUCCESS: Color32 = Color32::from_rgb(85, 176, 85);
    pub const ACCENT: Color32 = Color32::from_rgb(85, 153, 221);
}

/// Card dimensions for positioning calculations.
const CARD_WIDTH: f32 = 200.0;
const CARD_HEIGHT: f32 = 180.0; // Approximate height
const CARD_MARGIN: f32 = 25.0; // Distance from object
const DOCK_HEIGHT: f32 = 56.0;
const VELOCITY_ARROW_LENGTH: f32 = 35.0; // Approximate max arrow length to avoid

/// 8-position label placement algorithm.
/// Tries 8 candidate positions around the object and picks the best one.
/// Based on cartographic label placement principles.
fn smart_card_position(ctx: &egui::Context, screen_pos: Vec2, velocity_dir: Option<Vec2>) -> egui::Pos2 {
    let screen = ctx.screen_rect();
    let usable_bottom = screen.bottom() - DOCK_HEIGHT - 10.0;

    // Define 8 candidate positions: E, NE, N, NW, W, SW, S, SE
    // Each is (offset_x, offset_y) from screen_pos to card's top-left corner
    let candidates: [(f32, f32, &str); 8] = [
        (CARD_MARGIN, -CARD_HEIGHT / 2.0, "E"),                           // East (right-center)
        (CARD_MARGIN, -CARD_HEIGHT - CARD_MARGIN, "NE"),                  // Northeast
        (-CARD_WIDTH / 2.0, -CARD_HEIGHT - CARD_MARGIN, "N"),             // North (top-center)
        (-CARD_WIDTH - CARD_MARGIN, -CARD_HEIGHT - CARD_MARGIN, "NW"),    // Northwest
        (-CARD_WIDTH - CARD_MARGIN, -CARD_HEIGHT / 2.0, "W"),             // West (left-center)
        (-CARD_WIDTH - CARD_MARGIN, CARD_MARGIN, "SW"),                   // Southwest
        (-CARD_WIDTH / 2.0, CARD_MARGIN, "S"),                            // South (bottom-center)
        (CARD_MARGIN, CARD_MARGIN, "SE"),                                 // Southeast
    ];

    let mut best_score = f32::MIN;
    let mut best_pos = egui::pos2(screen_pos.x + CARD_MARGIN, screen_pos.y - CARD_HEIGHT / 2.0);

    for (offset_x, offset_y, _name) in candidates {
        let card_left = screen_pos.x + offset_x;
        let card_top = screen_pos.y + offset_y;
        let card_right = card_left + CARD_WIDTH;
        let card_bottom = card_top + CARD_HEIGHT;

        let mut score: f32 = 100.0;

        // Penalty: off-screen left
        if card_left < screen.left() {
            score -= (screen.left() - card_left) * 2.0;
        }
        // Penalty: off-screen right
        if card_right > screen.right() {
            score -= (card_right - screen.right()) * 2.0;
        }
        // Penalty: off-screen top
        if card_top < screen.top() {
            score -= (screen.top() - card_top) * 2.0;
        }
        // Penalty: overlaps dock area
        if card_bottom > usable_bottom {
            score -= (card_bottom - usable_bottom) * 3.0;
        }

        // Penalty: overlaps velocity arrow zone
        if let Some(vel_dir) = velocity_dir {
            // Calculate the velocity arrow endpoint
            let arrow_end_x = screen_pos.x + vel_dir.x * VELOCITY_ARROW_LENGTH;
            let arrow_end_y = screen_pos.y + vel_dir.y * VELOCITY_ARROW_LENGTH;

            // Check if arrow endpoint or midpoint is inside the card
            let arrow_mid_x = screen_pos.x + vel_dir.x * VELOCITY_ARROW_LENGTH * 0.5;
            let arrow_mid_y = screen_pos.y + vel_dir.y * VELOCITY_ARROW_LENGTH * 0.5;

            let endpoint_inside = arrow_end_x >= card_left && arrow_end_x <= card_right
                && arrow_end_y >= card_top && arrow_end_y <= card_bottom;
            let midpoint_inside = arrow_mid_x >= card_left && arrow_mid_x <= card_right
                && arrow_mid_y >= card_top && arrow_mid_y <= card_bottom;

            if endpoint_inside {
                score -= 80.0; // Heavy penalty - this blocks the drag handle
            }
            if midpoint_inside {
                score -= 40.0; // Medium penalty
            }

            // Bonus: position is on opposite side from velocity
            let card_center_x = card_left + CARD_WIDTH / 2.0;
            let card_center_y = card_top + CARD_HEIGHT / 2.0;
            let to_card_x = card_center_x - screen_pos.x;
            let to_card_y = card_center_y - screen_pos.y;
            let dot = to_card_x * vel_dir.x + to_card_y * vel_dir.y;
            if dot < 0.0 {
                score += 20.0; // Bonus for being on opposite side
            }
        }

        // Slight preference for right side (reading direction)
        if offset_x > 0.0 {
            score += 5.0;
        }

        if score > best_score {
            best_score = score;
            best_pos = egui::pos2(card_left, card_top);
        }
    }

    // Final clamp to ensure card stays on screen
    let final_x = best_pos.x.clamp(screen.left() + 5.0, screen.right() - CARD_WIDTH - 5.0);
    let final_y = best_pos.y.clamp(screen.top() + 5.0, usable_bottom - CARD_HEIGHT);

    egui::pos2(final_x, final_y)
}

/// System to render the context card near selected objects.
#[allow(clippy::too_many_arguments)]
pub fn context_card_system(
    mut commands: Commands,
    mut contexts: EguiContexts,
    selected: Res<SelectedBody>,
    camera_query: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    celestial_bodies: Query<(Entity, &CelestialBody)>,
    mut asteroids: Query<(Entity, &AsteroidName, &mut BodyState, &Transform), With<Asteroid>>,
    deflectors: Query<&ContinuousDeflector>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut radial_menu_state: ResMut<RadialMenuState>,
) {
    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    let Ok((camera, camera_transform)) = camera_query.get_single() else {
        return;
    };

    match selected.body {
        Some(SelectableBody::Celestial(entity)) => {
            if let Ok((_, body)) = celestial_bodies.get(entity) {
                // Get body position
                if let Some(pos) = ephemeris.get_position_by_id(body.id, sim_time.current) {
                    let render_pos = Vec2::new(
                        (pos.x * RENDER_SCALE) as f32,
                        (pos.y * RENDER_SCALE) as f32,
                    );

                    if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, render_pos.extend(0.0)) {
                        render_celestial_card(ctx, body, pos.length(), screen_pos, None);
                    }
                }
            }
        }
        Some(SelectableBody::Asteroid(entity)) => {
            if let Ok((_, name, mut body_state, transform)) = asteroids.get_mut(entity) {
                let render_pos = transform.translation.truncate();

                if let Ok(screen_pos) = camera.world_to_viewport(camera_transform, render_pos.extend(0.0)) {
                    // Check for active deflector on this asteroid
                    let active_deflector = deflectors.iter().find(|d| {
                        d.target == entity && (d.is_operating() || matches!(d.state, ContinuousDeflectorState::EnRoute { .. }))
                    });

                    // Get velocity direction for smart card positioning
                    let vel_dir = if body_state.vel.length() > 1.0 {
                        let normalized = body_state.vel.normalize();
                        Some(Vec2::new(normalized.x as f32, normalized.y as f32))
                    } else {
                        None
                    };

                    let result = render_asteroid_card(
                        ctx,
                        &name.0,
                        &mut body_state,
                        screen_pos,
                        active_deflector,
                        vel_dir,
                    );

                    if result.delete_clicked {
                        commands.entity(entity).despawn();
                        integrator_states.remove(entity);
                    }

                    if result.deflect_clicked {
                        radial_menu_state.open = true;
                        radial_menu_state.target = Some(entity);
                        radial_menu_state.position = screen_pos;
                    }
                }
            }
        }
        None => {}
    }
}

/// Result from rendering asteroid card.
struct AsteroidCardResult {
    delete_clicked: bool,
    deflect_clicked: bool,
}

/// Render context card for a celestial body.
fn render_celestial_card(
    ctx: &egui::Context,
    body: &CelestialBody,
    distance_from_sun: f64,
    screen_pos: Vec2,
    velocity_dir: Option<Vec2>,
) {
    // Smart positioning to avoid obstructing the object
    let card_pos = smart_card_position(ctx, screen_pos, velocity_dir);

    egui::Window::new("Selected Body")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .fixed_pos(card_pos)
        .frame(
            egui::Frame::none()
                .fill(colors::CARD_BG)
                .inner_margin(12.0)
                .stroke(egui::Stroke::new(1.0, colors::CARD_BORDER))
                .rounding(4.0),
        )
        .show(ctx, |ui| {
            ui.set_max_width(180.0);

            // Header with icon and name
            ui.horizontal(|ui| {
                let icon = celestial_icon(body.id);
                ui.label(egui::RichText::new(icon).size(16.0)); // Phosphor icon
                ui.label(egui::RichText::new(&body.name).strong().size(16.0));
            });

            ui.separator();

            // Type
            let body_type = celestial_type(body.id);
            ui.label(egui::RichText::new(body_type).weak().size(13.0));

            // Distance from Sun
            let dist_au = distance_from_sun / AU_TO_METERS;
            let dist_mkm = distance_from_sun / 1e9;
            ui.label(egui::RichText::new(format!("{:.1} M km from Sun", dist_mkm)).size(14.0));
            ui.label(egui::RichText::new(format!("({:.3} AU)", dist_au)).weak().size(12.0));
        });
}

/// Render context card for an asteroid.
fn render_asteroid_card(
    ctx: &egui::Context,
    name: &str,
    body_state: &mut BodyState,
    screen_pos: Vec2,
    active_deflector: Option<&ContinuousDeflector>,
    velocity_dir: Option<Vec2>,
) -> AsteroidCardResult {
    use crate::ui::icons;

    let mut result = AsteroidCardResult {
        delete_clicked: false,
        deflect_clicked: false,
    };

    // Smart positioning to avoid obstructing the object AND velocity arrow
    let card_pos = smart_card_position(ctx, screen_pos, velocity_dir);

    egui::Window::new("Asteroid")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .fixed_pos(card_pos)
        .frame(
            egui::Frame::none()
                .fill(colors::CARD_BG)
                .inner_margin(12.0)
                .stroke(egui::Stroke::new(1.0, colors::CARD_BORDER))
                .rounding(4.0),
        )
        .show(ctx, |ui| {
            ui.set_max_width(200.0);

            // Header with icon and name
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icons::ASTEROID).size(16.0));
                ui.label(egui::RichText::new(name).strong().size(16.0));
            });

            ui.separator();

            // Key stats
            let dist_from_sun = body_state.pos.length();
            let dist_mkm = dist_from_sun / 1e9;
            let vel_km_s = body_state.vel.length() / 1000.0;

            ui.label(egui::RichText::new(format!("{:.1} M km from Sun", dist_mkm)).size(14.0));
            ui.label(egui::RichText::new(format!("{:.1} km/s velocity", vel_km_s)).size(14.0));

            // Editable mass field
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Mass:").size(14.0));

                // Convert mass to mantissa and exponent for editing
                let exponent = body_state.mass.log10().floor();
                let mantissa = body_state.mass / 10_f64.powf(exponent);
                let mut mantissa_f32 = mantissa as f32;
                let mut exponent_i32 = exponent as i32;

                // Mantissa drag value (1.0 - 9.99)
                let mantissa_changed = ui.add(
                    egui::DragValue::new(&mut mantissa_f32)
                        .range(1.0..=9.99)
                        .speed(0.01)
                        .fixed_decimals(2)
                ).on_hover_text("Drag to adjust mass").changed();

                ui.label(egui::RichText::new("×10").size(12.0));

                // Exponent drag value
                let exponent_changed = ui.add(
                    egui::DragValue::new(&mut exponent_i32)
                        .range(6..=18)
                        .speed(0.1)
                ).on_hover_text("Mass exponent (kg)").changed();

                ui.label(egui::RichText::new("kg").size(12.0));

                // Update mass if changed
                if mantissa_changed || exponent_changed {
                    body_state.mass = (mantissa_f32 as f64) * 10_f64.powf(exponent_i32 as f64);
                }
            });

            // Active deflection section
            if let Some(deflector) = active_deflector {
                ui.add_space(4.0);
                ui.separator();
                render_deflection_status(ui, deflector);
            }

            ui.add_space(8.0);
            ui.separator();

            // Action buttons
            ui.horizontal(|ui| {
                let deflect_button = egui::Button::new(
                    egui::RichText::new("Deflect >").size(14.0).color(egui::Color32::WHITE)
                ).fill(colors::ACCENT).min_size(egui::vec2(75.0, 28.0));

                if ui.add(deflect_button).clicked() {
                    result.deflect_clicked = true;
                }

                let delete_button = egui::Button::new(
                    egui::RichText::new("Delete").size(14.0).color(egui::Color32::WHITE)
                ).fill(colors::DANGER).min_size(egui::vec2(60.0, 28.0));

                if ui.add(delete_button).clicked() {
                    result.delete_clicked = true;
                }
            });
        });

    result
}

/// Render active deflection mission status.
fn render_deflection_status(ui: &mut egui::Ui, deflector: &ContinuousDeflector) {
    use crate::ui::icons;

    ui.label(egui::RichText::new("ACTIVE DEFLECTION").strong().size(12.0).color(colors::SUCCESS));

    let (icon, method_name) = deflector_display(&deflector.payload);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(icon).size(14.0));
        ui.label(method_name);
    });

    match &deflector.state {
        ContinuousDeflectorState::EnRoute { .. } => {
            ui.label(egui::RichText::new(format!("{} En route", icons::CLOCK)).weak().size(12.0));
        }
        ContinuousDeflectorState::Operating { fuel_consumed, accumulated_delta_v, .. } => {
            // Fuel bar if applicable
            if let Some(initial_fuel) = deflector.payload.initial_fuel() {
                let remaining = (initial_fuel - fuel_consumed).max(0.0);
                let fraction = remaining / initial_fuel;

                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Fuel:").size(12.0));
                    ui.add(egui::ProgressBar::new(fraction as f32).desired_width(80.0));
                });
            }

            ui.label(format!("Dv: +{:.4} mm/s", accumulated_delta_v * 1000.0));
        }
        ContinuousDeflectorState::FuelDepleted { total_delta_v, .. } => {
            ui.label(egui::RichText::new(format!("{} Fuel depleted", icons::FUEL)).weak().size(12.0));
            ui.label(format!("Total Dv: {:.4} mm/s", total_delta_v * 1000.0));
        }
        ContinuousDeflectorState::Complete { total_delta_v, .. } => {
            ui.label(egui::RichText::new(format!("{} Complete", icons::SUCCESS)).color(colors::SUCCESS).size(12.0));
            ui.label(format!("Total Dv: {:.4} mm/s", total_delta_v * 1000.0));
        }
        ContinuousDeflectorState::Cancelled => {
            ui.label(egui::RichText::new(format!("{} Cancelled", icons::CLOSE)).color(colors::DANGER).size(12.0));
        }
    }
}

/// Get display info for a deflector payload.
fn deflector_display(payload: &crate::continuous::ContinuousPayload) -> (&'static str, &'static str) {
    use crate::continuous::ContinuousPayload;
    use crate::ui::icons;
    match payload {
        ContinuousPayload::IonBeam { .. } => (icons::ION_BEAM, "Ion Beam"),
        ContinuousPayload::GravityTractor { .. } => (icons::GRAVITY_TRACTOR, "Gravity Tractor"),
        ContinuousPayload::LaserAblation { .. } => (icons::LASER, "Laser Ablation"),
        ContinuousPayload::SolarSail { .. } => (icons::SOLAR_SAIL, "Solar Sail"),
    }
}

/// Get icon for a celestial body.
fn celestial_icon(id: CelestialBodyId) -> &'static str {
    use crate::ui::icons;
    match id {
        CelestialBodyId::Sun => icons::SUN,
        _ if CelestialBodyId::PLANETS.contains(&id) => icons::PLANET,
        _ => icons::MOON, // Moons
    }
}

/// Get type description for a celestial body.
fn celestial_type(id: CelestialBodyId) -> &'static str {
    if id == CelestialBodyId::Sun {
        "Star"
    } else if CelestialBodyId::PLANETS.contains(&id) {
        "Planet"
    } else if CelestialBodyId::MOONS.contains(&id) {
        "Moon"
    } else {
        "Body"
    }
}
