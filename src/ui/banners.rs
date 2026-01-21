//! Outcome banners - non-blocking notifications for trajectory outcomes.
//!
//! Banners slide in from the top to show collision predictions, escape
//! trajectories, and stable orbits without blocking viewport interaction.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::asteroid::{Asteroid, ResetEvent};
use crate::collision::{CollisionEvent, CollisionState};
use crate::continuous::{ContinuousPayload, LaunchContinuousDeflectorEvent, ThrustDirection};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::interceptor::{DeflectionPayload, LaunchInterceptorEvent};
use crate::outcome::TrajectoryOutcome;
use crate::prediction::TrajectoryPath;
use crate::render::SelectedBody;
use crate::scenarios::CurrentScenario;
use crate::types::{AU_TO_METERS, BodyState, SECONDS_PER_DAY, SelectableBody, SimulationTime};

use super::icons;
use super::radial_menu::DeflectionMethod;
use super::{RadialMenuState, ScenarioDrawerState};

/// Base interceptor speed in m/s (for flight time calculation).
const BASE_INTERCEPTOR_SPEED: f64 = 15_000.0;

/// All deflection methods available in the inline strip.
const ALL_METHODS: [DeflectionMethod; 7] = [
    DeflectionMethod::Kinetic,
    DeflectionMethod::Nuclear,
    DeflectionMethod::NuclearSplit,
    DeflectionMethod::IonBeam,
    DeflectionMethod::GravityTractor,
    DeflectionMethod::LaserAblation,
    DeflectionMethod::SolarSail,
];

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
    asteroids: Query<&BodyState, With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    mut launch_events: MessageWriter<LaunchInterceptorEvent>,
    mut continuous_launch_events: MessageWriter<LaunchContinuousDeflectorEvent>,
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
                // Get asteroid body state for deflection
                let asteroid_data = if let Some(SelectableBody::Asteroid(entity)) = selected.body {
                    asteroids.get(entity).ok().map(|bs| (entity, bs.clone()))
                } else {
                    None
                };

                // Calculate flight time from Earth
                let flight_time_days = if let Some((_, ref body_state)) = asteroid_data {
                    let earth_pos = ephemeris
                        .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
                        .unwrap_or(bevy::math::DVec2::new(AU_TO_METERS, 0.0));
                    let distance = (body_state.pos - earth_pos).length();
                    let flight_time_seconds = distance / BASE_INTERCEPTOR_SPEED;
                    flight_time_seconds / SECONDS_PER_DAY
                } else {
                    0.0
                };

                render_collision_prediction_banner(
                    ctx,
                    *body_hit,
                    *time_to_impact,
                    *impact_velocity,
                    &mut reset_events,
                    &mut radial_menu_state,
                    asteroid_data,
                    flight_time_days,
                    &mut launch_events,
                    &mut continuous_launch_events,
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
                ui.label(icons::icon_colored(
                    icons::WARNING,
                    18.0,
                    colors::COLLISION_BORDER,
                ));
                ui.add_space(8.0);

                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} IMPACTED {}",
                            collision.asteroid_name,
                            collision.body_hit.name()
                        ))
                        .strong()
                        .size(16.0)
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

/// Render collision prediction banner with inline deflection strip.
#[allow(clippy::too_many_arguments)]
fn render_collision_prediction_banner(
    ctx: &egui::Context,
    body_hit: crate::ephemeris::CelestialBodyId,
    time_to_impact: f64,
    impact_velocity: f64,
    reset_events: &mut MessageWriter<ResetEvent>,
    radial_menu_state: &mut RadialMenuState,
    asteroid_data: Option<(Entity, BodyState)>,
    flight_time_days: f64,
    launch_events: &mut MessageWriter<LaunchInterceptorEvent>,
    continuous_launch_events: &mut MessageWriter<LaunchContinuousDeflectorEvent>,
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
                ui.label(icons::icon_colored(
                    icons::WARNING,
                    18.0,
                    colors::COLLISION_BORDER,
                ));
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

                ui.separator();

                // Inline deflection strip (if asteroid is selected)
                if let Some((entity, ref body_state)) = asteroid_data {
                    ui.label(egui::RichText::new("Deflect:").size(12.0));

                    for method in ALL_METHODS.iter() {
                        let icon_str = method.icon();
                        let color = method.color();

                        let btn = ui
                            .add_sized(
                                [28.0, 28.0],
                                egui::Button::new(icons::icon_colored(
                                    icon_str,
                                    14.0,
                                    egui::Color32::WHITE,
                                ))
                                .fill(egui::Color32::from_rgba_premultiplied(
                                    color.r() / 2,
                                    color.g() / 2,
                                    color.b() / 2,
                                    200,
                                ))
                                .stroke(egui::Stroke::new(1.0, color)),
                            )
                            .on_hover_text(method.name());

                        if btn.clicked() {
                            let flight_time_seconds = flight_time_days * SECONDS_PER_DAY;
                            apply_deflection(
                                entity,
                                *method,
                                body_state,
                                flight_time_seconds,
                                launch_events,
                                continuous_launch_events,
                            );
                            // Also set radial menu state for potential right-click
                            radial_menu_state.target = Some(entity);
                        }
                    }

                    ui.label(
                        egui::RichText::new(format!("~{:.0}d", flight_time_days))
                            .weak()
                            .size(11.0),
                    );
                }

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
                ui.label(icons::icon_colored(
                    icons::ARROW_RIGHT,
                    18.0,
                    colors::ESCAPE_BORDER,
                ));
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
                ui.label(icons::icon_colored(
                    icons::SUCCESS,
                    18.0,
                    colors::STABLE_BORDER,
                ));
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
                    ui.horizontal(|ui| {
                        ui.label(icons::icon_colored(
                            icons::SUCCESS,
                            15.0,
                            colors::STABLE_BORDER,
                        ));
                        ui.label(
                            egui::RichText::new("Earth is safe!")
                                .color(colors::STABLE_BORDER)
                                .strong()
                                .size(15.0),
                        );
                    });
                }
            });
        });
}

/// Apply a deflection method with default parameters.
fn apply_deflection(
    target: Entity,
    method: DeflectionMethod,
    asteroid_state: &BodyState,
    flight_time_seconds: f64,
    launch_events: &mut MessageWriter<LaunchInterceptorEvent>,
    continuous_launch_events: &mut MessageWriter<LaunchContinuousDeflectorEvent>,
) {
    // Default direction: retrograde (opposite to velocity)
    let direction = -asteroid_state.vel.normalize_or_zero();

    match method {
        DeflectionMethod::Kinetic => {
            launch_events.write(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::Kinetic {
                    mass_kg: 560.0,
                    beta: 3.6,
                },
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::Nuclear => {
            launch_events.write(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::Nuclear { yield_kt: 100.0 },
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::NuclearSplit => {
            launch_events.write(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::NuclearSplit {
                    yield_kt: 500.0,
                    split_ratio: 0.5,
                },
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::IonBeam => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::IonBeam {
                    thrust_n: 0.1,
                    fuel_mass_kg: 500.0,
                    specific_impulse: 3500.0,
                    hover_distance_m: 200.0,
                    direction: ThrustDirection::Retrograde,
                },
                flight_time: flight_time_seconds,
            });
        }
        DeflectionMethod::GravityTractor => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::GravityTractor {
                    spacecraft_mass_kg: 20_000.0,
                    hover_distance_m: 200.0,
                    mission_duration: 10.0 * 365.25 * 86400.0,
                    direction: ThrustDirection::Retrograde,
                },
                flight_time: flight_time_seconds,
            });
        }
        DeflectionMethod::LaserAblation => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::LaserAblation {
                    power_kw: 100.0,
                    mission_duration: 12.0 * 30.44 * 86400.0,
                    efficiency: 0.8,
                    direction: ThrustDirection::Retrograde,
                },
                flight_time: flight_time_seconds,
            });
        }
        DeflectionMethod::SolarSail => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::SolarSail {
                    sail_area_m2: 10_000.0,
                    mission_duration: 2.0 * 365.25 * 86400.0,
                    reflectivity: 0.9,
                    direction: ThrustDirection::SunPointing,
                },
                flight_time: flight_time_seconds,
            });
        }
    }
}
