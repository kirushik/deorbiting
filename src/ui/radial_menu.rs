//! Radial deflection menu - in-place tool selector.
//!
//! When "Deflect" is clicked on a context card, a radial menu appears
//! around the asteroid showing all deflection method options.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::asteroid::Asteroid;
use crate::continuous::{ContinuousPayload, LaunchContinuousDeflectorEvent, ThrustDirection};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::interceptor::{DeflectionPayload, LaunchInterceptorEvent};
use crate::types::{AU_TO_METERS, BodyState, SECONDS_PER_DAY, SimulationTime};

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

/// Available deflection methods.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeflectionMethod {
    // Instant methods
    Kinetic,
    Nuclear,
    NuclearSplit,
    // Continuous methods
    IonBeam,
    GravityTractor,
    LaserAblation,
    SolarSail,
}

impl DeflectionMethod {
    fn icon(&self) -> &'static str {
        use crate::ui::icons;
        match self {
            DeflectionMethod::Kinetic => icons::KINETIC,
            DeflectionMethod::Nuclear => icons::NUCLEAR,
            DeflectionMethod::NuclearSplit => icons::NUCLEAR_SPLIT,
            DeflectionMethod::IonBeam => icons::ION_BEAM,
            DeflectionMethod::GravityTractor => icons::GRAVITY_TRACTOR,
            DeflectionMethod::LaserAblation => icons::LASER,
            DeflectionMethod::SolarSail => icons::SOLAR_SAIL,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            DeflectionMethod::Kinetic => "Kinetic",
            DeflectionMethod::Nuclear => "Nuclear",
            DeflectionMethod::NuclearSplit => "Split",
            DeflectionMethod::IonBeam => "Ion Beam",
            DeflectionMethod::GravityTractor => "Gravity",
            DeflectionMethod::LaserAblation => "Laser",
            DeflectionMethod::SolarSail => "Solar Sail",
        }
    }

    fn color(&self) -> egui::Color32 {
        match self {
            DeflectionMethod::Kinetic => egui::Color32::from_rgb(255, 180, 100),
            DeflectionMethod::Nuclear => egui::Color32::from_rgb(255, 100, 100),
            DeflectionMethod::NuclearSplit => egui::Color32::from_rgb(255, 80, 150),
            DeflectionMethod::IonBeam => egui::Color32::from_rgb(100, 200, 255),
            DeflectionMethod::GravityTractor => egui::Color32::from_rgb(180, 120, 255),
            DeflectionMethod::LaserAblation => egui::Color32::from_rgb(255, 200, 80),
            DeflectionMethod::SolarSail => egui::Color32::from_rgb(255, 230, 100),
        }
    }

    fn is_continuous(&self) -> bool {
        matches!(
            self,
            DeflectionMethod::IonBeam
                | DeflectionMethod::GravityTractor
                | DeflectionMethod::LaserAblation
                | DeflectionMethod::SolarSail
        )
    }
}

/// Colors for the radial menu.
mod colors {
    use bevy_egui::egui::Color32;

    pub const MENU_BG: Color32 = Color32::from_rgba_premultiplied(26, 26, 36, 230);
    pub const BUTTON_BG: Color32 = Color32::from_rgba_premultiplied(50, 50, 70, 255);
    pub const BUTTON_HOVER: Color32 = Color32::from_rgba_premultiplied(70, 70, 100, 255);
}

/// Base interceptor speed in m/s.
const BASE_INTERCEPTOR_SPEED: f64 = 15_000.0;

/// All deflection methods to show in the radial menu.
const ALL_METHODS: [DeflectionMethod; 7] = [
    DeflectionMethod::Kinetic,
    DeflectionMethod::Nuclear,
    DeflectionMethod::NuclearSplit,
    DeflectionMethod::IonBeam,
    DeflectionMethod::GravityTractor,
    DeflectionMethod::LaserAblation,
    DeflectionMethod::SolarSail,
];

/// System to render the radial deflection menu.
#[allow(clippy::too_many_arguments)]
pub fn radial_menu_system(
    mut contexts: EguiContexts,
    mut menu_state: ResMut<RadialMenuState>,
    mut launch_events: EventWriter<LaunchInterceptorEvent>,
    mut continuous_launch_events: EventWriter<LaunchContinuousDeflectorEvent>,
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

    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    // Calculate distance to Earth for flight time
    let earth_pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
        .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
    let distance = (asteroid_state.pos - earth_pos).length();
    let flight_time_seconds = distance / BASE_INTERCEPTOR_SPEED;
    let flight_time_days = flight_time_seconds / SECONDS_PER_DAY;

    let center = egui::pos2(menu_state.position.x, menu_state.position.y);
    let radius = 90.0;

    // Draw the radial menu
    egui::Area::new(egui::Id::new("radial_menu"))
        .fixed_pos(center - egui::vec2(radius + 50.0, radius + 50.0))
        .show(ctx, |ui| {
            // Background circle
            ui.painter().circle(
                center,
                radius + 40.0,
                colors::MENU_BG,
                egui::Stroke::new(2.0, egui::Color32::from_rgb(60, 60, 80)),
            );

            // Flight time indicator at center
            ui.painter().text(
                center - egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                format!("{:.0}d", flight_time_days),
                egui::FontId::proportional(16.0),
                egui::Color32::from_rgb(180, 180, 190),
            );
            ui.painter().text(
                center + egui::vec2(0.0, 8.0),
                egui::Align2::CENTER_CENTER,
                "flight",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(120, 120, 130),
            );

            // Arrange all 7 methods in a circle
            let num_methods = ALL_METHODS.len();
            for (i, method) in ALL_METHODS.iter().enumerate() {
                // Start from top and go clockwise
                let angle = -std::f32::consts::FRAC_PI_2
                    + (i as f32 * std::f32::consts::TAU / num_methods as f32);
                let button_pos = center + egui::vec2(angle.cos() * radius, angle.sin() * radius);

                // Show keyboard shortcut number (1-7)
                let shortcut_key = (i + 1).to_string();
                if render_method_button(ui, button_pos, *method, &shortcut_key) {
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

    // Close on click outside
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

    // Keyboard shortcuts for quick selection (1-7)
    let key_method_map = [
        (egui::Key::Num1, DeflectionMethod::Kinetic),
        (egui::Key::Num2, DeflectionMethod::Nuclear),
        (egui::Key::Num3, DeflectionMethod::NuclearSplit),
        (egui::Key::Num4, DeflectionMethod::IonBeam),
        (egui::Key::Num5, DeflectionMethod::GravityTractor),
        (egui::Key::Num6, DeflectionMethod::LaserAblation),
        (egui::Key::Num7, DeflectionMethod::SolarSail),
    ];

    for (key, method) in key_method_map {
        if ctx.input(|i| i.key_pressed(key)) {
            apply_deflection(
                target,
                method,
                asteroid_state,
                flight_time_seconds,
                &mut launch_events,
                &mut continuous_launch_events,
            );
            menu_state.open = false;
            break;
        }
    }
}

/// Render a method button. Returns true if clicked.
fn render_method_button(
    ui: &mut egui::Ui,
    pos: egui::Pos2,
    method: DeflectionMethod,
    shortcut: &str,
) -> bool {
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
    ui.painter()
        .rect(rect, 6.0, bg_color, egui::Stroke::new(2.0, accent_color));

    // Keyboard shortcut badge (top-left corner)
    ui.painter().text(
        pos + egui::vec2(-button_size / 2.0 + 6.0, -button_size / 2.0 + 8.0),
        egui::Align2::LEFT_TOP,
        shortcut,
        egui::FontId::proportional(10.0),
        egui::Color32::from_rgb(150, 150, 160),
    );

    // Icon (use proportional font where Phosphor is added as fallback)
    ui.painter().text(
        pos,
        egui::Align2::CENTER_CENTER,
        method.icon(),
        egui::FontId::proportional(20.0),
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
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(130, 130, 140),
    );

    response.clicked()
}

/// Apply a deflection method with default parameters.
fn apply_deflection(
    target: Entity,
    method: DeflectionMethod,
    asteroid_state: &BodyState,
    flight_time_seconds: f64,
    launch_events: &mut EventWriter<LaunchInterceptorEvent>,
    continuous_launch_events: &mut EventWriter<LaunchContinuousDeflectorEvent>,
) {
    // Default direction: retrograde (opposite to velocity)
    let direction = -asteroid_state.vel.normalize_or_zero();

    match method {
        DeflectionMethod::Kinetic => {
            launch_events.send(LaunchInterceptorEvent {
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
            launch_events.send(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::Nuclear { yield_kt: 100.0 },
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::NuclearSplit => {
            launch_events.send(LaunchInterceptorEvent {
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
            continuous_launch_events.send(LaunchContinuousDeflectorEvent {
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
            continuous_launch_events.send(LaunchContinuousDeflectorEvent {
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
            continuous_launch_events.send(LaunchContinuousDeflectorEvent {
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
            continuous_launch_events.send(LaunchContinuousDeflectorEvent {
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
