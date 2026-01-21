//! Outcome banners - non-blocking notifications for trajectory outcomes.
//!
//! Banners slide in from the top to show collision predictions, escape
//! trajectories, and stable orbits without blocking viewport interaction.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::asteroid::ResetEvent;
use crate::collision::{CollisionEvent, CollisionState};
use crate::outcome::TrajectoryOutcome;
use crate::prediction::TrajectoryPath;
use crate::render::SelectedBody;
use crate::scenarios::CurrentScenario;
use crate::types::{AU_TO_METERS, SECONDS_PER_DAY, SelectableBody, SimulationTime};

use super::icons;
use super::{RadialMenuState, ScenarioDrawerState};

/// Resource for banner state.
#[derive(Resource, Default)]
pub struct BannerState {
    /// Current trajectory outcome being displayed.
    pub outcome: Option<TrajectoryOutcome>,
    /// Current collision notification (from actual collision events).
    pub collision: Option<CollisionEvent>,
    /// Animation progress for collision banner (0.0 to 1.0).
    pub collision_flash: f32,
}

/// Colors for banners.
mod colors {
    use bevy_egui::egui::Color32;

    pub const COLLISION_BG: Color32 = Color32::from_rgba_premultiplied(80, 30, 30, 240);
    pub const COLLISION_BORDER: Color32 = Color32::from_rgb(224, 85, 85);
    pub const ESCAPE_BG: Color32 = Color32::from_rgba_premultiplied(30, 50, 80, 240);
    pub const ESCAPE_BORDER: Color32 = Color32::from_rgb(85, 153, 221);
    pub const STABLE_BG: Color32 = Color32::from_rgba_premultiplied(30, 60, 40, 240);
    pub const STABLE_BORDER: Color32 = Color32::from_rgb(85, 176, 85);
}

/// System to update banner state from selected asteroid trajectory.
pub fn update_banner_state(
    selected: Res<SelectedBody>,
    trajectories: Query<&TrajectoryPath>,
    mut collision_state: ResMut<CollisionState>,
    mut banner_state: ResMut<BannerState>,
    sim_time: Res<SimulationTime>,
) {
    // Handle collision notifications from actual collisions
    if sim_time.paused
        && banner_state.collision.is_none()
        && let Some(collision) = collision_state.pop_notification()
    {
        banner_state.collision = Some(collision);
        banner_state.collision_flash = 1.0;
    }

    // Clear collision notification when simulation resumes
    if !sim_time.paused {
        banner_state.collision = None;
    }

    // Get trajectory outcome for selected asteroid
    let outcome = if let Some(SelectableBody::Asteroid(entity)) = selected.body {
        if let Ok(trajectory) = trajectories.get(entity) {
            if trajectory.outcome.is_determined() {
                Some(trajectory.outcome.clone())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    banner_state.outcome = outcome;
}

/// System to animate banner effects.
pub fn animate_banners(time: Res<Time>, mut banner_state: ResMut<BannerState>) {
    if banner_state.collision_flash > 0.0 {
        banner_state.collision_flash -= time.delta_secs() / 0.3;
        banner_state.collision_flash = banner_state.collision_flash.max(0.0);
    }
}

/// System to render outcome banners.
#[allow(clippy::too_many_arguments)]
pub fn banner_system(
    mut contexts: EguiContexts,
    banner_state: Res<BannerState>,
    current_scenario: Res<CurrentScenario>,
    mut reset_events: MessageWriter<ResetEvent>,
    _drawer_state: ResMut<ScenarioDrawerState>,
    mut radial_menu_state: ResMut<RadialMenuState>,
    selected: Res<SelectedBody>,
) {
    let Some(ctx) = contexts.ctx_mut().ok() else {
        return;
    };

    // Render collision notification (actual collision, highest priority)
    if let Some(collision) = &banner_state.collision {
        render_collision_notification(
            ctx,
            collision,
            banner_state.collision_flash,
            &mut reset_events,
        );
        return; // Don't show outcome banner if collision notification is showing
    }

    // Render trajectory outcome banner
    if let Some(outcome) = &banner_state.outcome {
        match outcome {
            TrajectoryOutcome::Collision {
                body_hit,
                time_to_impact,
                impact_velocity,
            } => {
                render_collision_prediction_banner(
                    ctx,
                    *body_hit,
                    *time_to_impact,
                    *impact_velocity,
                    &mut reset_events,
                    &mut radial_menu_state,
                    &selected,
                );
            }
            TrajectoryOutcome::Escape {
                v_infinity,
                direction,
            } => {
                render_escape_banner(ctx, *v_infinity, *direction);
            }
            TrajectoryOutcome::StableOrbit {
                semi_major_axis,
                eccentricity,
                period,
                ..
            } => {
                render_stable_orbit_banner(
                    ctx,
                    *semi_major_axis,
                    *eccentricity,
                    *period,
                    current_scenario.id == "deflection_challenge",
                );
            }
            TrajectoryOutcome::InProgress => {}
        }
    }
}

/// Render actual collision notification (pauses simulation).
fn render_collision_notification(
    ctx: &egui::Context,
    collision: &CollisionEvent,
    flash_progress: f32,
    reset_events: &mut MessageWriter<ResetEvent>,
) {
    // Flash effect
    if flash_progress > 0.0 {
        let alpha = (flash_progress * 0.4 * 255.0) as u8;
        egui::Area::new(egui::Id::new("collision_flash"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.painter().rect_filled(
                    ui.ctx().viewport_rect(),
                    0.0,
                    egui::Color32::from_rgba_unmultiplied(255, 0, 0, alpha),
                );
            });
    }

    egui::TopBottomPanel::top("collision_notification")
        .frame(
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_premultiplied(60, 20, 20, 250))
                .inner_margin(egui::Margin::symmetric(16, 12))
                .stroke(egui::Stroke::new(2.0, colors::COLLISION_BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(icons::WARNING)
                        .size(20.0)
                        .color(colors::COLLISION_BORDER),
                );
                ui.add_space(8.0);

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} IMPACTED {}",
                            collision.asteroid_name,
                            collision.body_hit.name()
                        ))
                        .strong()
                        .size(18.0)
                        .color(egui::Color32::WHITE),
                    );
                    ui.label(
                        egui::RichText::new(format!(
                            "Impact velocity: {:.1} km/s",
                            collision.impact_speed_km_s()
                        ))
                        .size(14.0),
                    );
                });

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Reset")
                                    .size(14.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .min_size(egui::vec2(60.0, 28.0)),
                        )
                        .clicked()
                    {
                        reset_events.write(ResetEvent);
                    }
                });
            });
        });
}

/// Render collision prediction banner.
fn render_collision_prediction_banner(
    ctx: &egui::Context,
    body_hit: crate::ephemeris::CelestialBodyId,
    time_to_impact: f64,
    impact_velocity: f64,
    reset_events: &mut MessageWriter<ResetEvent>,
    radial_menu_state: &mut RadialMenuState,
    selected: &Res<SelectedBody>,
) {
    let days = time_to_impact / SECONDS_PER_DAY;
    let speed_km_s = impact_velocity / 1000.0;

    egui::TopBottomPanel::top("collision_banner")
        .frame(
            egui::Frame::NONE
                .fill(colors::COLLISION_BG)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .stroke(egui::Stroke::new(1.0, colors::COLLISION_BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(icons::WARNING)
                        .size(18.0)
                        .color(colors::COLLISION_BORDER),
                );
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new(format!(
                        "COLLISION in {:.0} days with {}",
                        days,
                        body_hit.name()
                    ))
                    .strong()
                    .size(16.0)
                    .color(egui::Color32::WHITE),
                );

                ui.separator();

                ui.label(egui::RichText::new(format!("Impact: {:.1} km/s", speed_km_s)).size(14.0));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Reset")
                                    .size(14.0)
                                    .color(egui::Color32::WHITE),
                            )
                            .min_size(egui::vec2(60.0, 28.0)),
                        )
                        .clicked()
                    {
                        reset_events.write(ResetEvent);
                    }

                    // Deflect button - opens radial menu
                    if let Some(SelectableBody::Asteroid(entity)) = selected.body
                        && ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new("Deflect")
                                        .size(14.0)
                                        .color(egui::Color32::WHITE),
                                )
                                .fill(egui::Color32::from_rgb(85, 153, 221))
                                .min_size(egui::vec2(70.0, 28.0)),
                            )
                            .clicked()
                    {
                        radial_menu_state.open = true;
                        radial_menu_state.target = Some(entity);
                        // Position will be set by context card or center of screen
                        radial_menu_state.position = Vec2::new(
                            ctx.viewport_rect().width() / 2.0,
                            ctx.viewport_rect().height() / 2.0,
                        );
                    }
                });
            });
        });
}

/// Render escape trajectory banner.
fn render_escape_banner(ctx: &egui::Context, v_infinity: f64, _direction: bevy::math::DVec2) {
    let v_km_s = v_infinity / 1000.0;

    egui::TopBottomPanel::top("escape_banner")
        .frame(
            egui::Frame::NONE
                .fill(colors::ESCAPE_BG)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .stroke(egui::Stroke::new(1.0, colors::ESCAPE_BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(icons::ARROW_RIGHT)
                        .size(18.0)
                        .color(colors::ESCAPE_BORDER),
                );
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new("ESCAPE TRAJECTORY")
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::WHITE),
                );

                ui.separator();

                ui.label(egui::RichText::new(format!("v_inf = {:.2} km/s", v_km_s)).size(14.0));

                ui.separator();

                ui.label(
                    egui::RichText::new("Leaving solar system")
                        .weak()
                        .size(14.0),
                );
            });
        });
}

/// Render stable orbit banner.
fn render_stable_orbit_banner(
    ctx: &egui::Context,
    semi_major_axis: f64,
    eccentricity: f64,
    period: f64,
    is_deflection_challenge: bool,
) {
    let a_au = semi_major_axis / AU_TO_METERS;
    let period_years = period / (365.25 * SECONDS_PER_DAY);

    egui::TopBottomPanel::top("stable_banner")
        .frame(
            egui::Frame::NONE
                .fill(colors::STABLE_BG)
                .inner_margin(egui::Margin::symmetric(16, 8))
                .stroke(egui::Stroke::new(1.0, colors::STABLE_BORDER)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new(icons::SUCCESS)
                        .size(18.0)
                        .color(colors::STABLE_BORDER),
                );
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new("STABLE ORBIT")
                        .strong()
                        .size(16.0)
                        .color(egui::Color32::WHITE),
                );

                ui.separator();

                ui.label(
                    egui::RichText::new(format!("Period: {:.2} years", period_years)).size(14.0),
                );

                ui.separator();

                ui.label(
                    egui::RichText::new(format!("a = {:.3} AU, e = {:.3}", a_au, eccentricity))
                        .weak()
                        .size(13.0),
                );

                if is_deflection_challenge {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("* Earth is safe! *")
                            .color(colors::STABLE_BORDER)
                            .strong()
                            .size(15.0),
                    );
                }
            });
        });
}
