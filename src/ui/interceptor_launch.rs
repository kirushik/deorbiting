//! Interceptor launch configuration modal.
//!
//! UI for configuring and launching interceptor missions:
//! - Payload type selection (Kinetic / Nuclear / Continuous methods)
//! - Parameter adjustment (mass, beta, yield, thrust, etc.)
//! - Propulsion technology selection (affects speed)
//! - Direction selection
//! - Delta-v preview

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::asteroid::Asteroid;
use crate::continuous::{ContinuousPayload, LaunchContinuousDeflectorEvent, ThrustDirection};
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::interceptor::{DeflectionPayload, LaunchInterceptorEvent};
use crate::outcome::TrajectoryOutcome;
use crate::prediction::TrajectoryPath;
use crate::render::SelectedBody;
use crate::types::{BodyState, SelectableBody, SimulationTime, AU_TO_METERS, SECONDS_PER_DAY};

/// Base interceptor speed in m/s.
/// ~15 km/s represents achievable speed with current chemical propulsion
/// for interplanetary transfers (similar to DART mission average).
/// Reference: DART traveled 11 million km in ~9 months.
const BASE_INTERCEPTOR_SPEED: f64 = 15_000.0; // 15 km/s

/// Resource for interceptor launch UI state.
#[derive(Resource)]
pub struct InterceptorLaunchState {
    /// Whether the launch window is open.
    pub open: bool,
    /// Selected payload type.
    pub payload_type: PayloadType,
    /// Kinetic impactor mass (kg).
    pub kinetic_mass: f64,
    /// Kinetic impactor beta factor.
    pub kinetic_beta: f64,
    /// Nuclear yield (kt).
    pub nuclear_yield: f64,
    /// Propulsion technology level (affects speed).
    pub propulsion_level: PropulsionLevel,
    /// Direction mode.
    pub direction_mode: DirectionMode,
    /// Custom direction angle (degrees from +X).
    pub custom_angle: f64,

    // Continuous deflection parameters (Phase 6)
    /// Ion beam thrust in millinewtons.
    pub ion_thrust_mn: f64,
    /// Ion beam fuel mass in kg.
    pub ion_fuel_mass_kg: f64,
    /// Ion beam specific impulse in seconds.
    pub ion_isp: f64,
    /// Gravity tractor spacecraft mass in kg.
    pub gt_spacecraft_mass_kg: f64,
    /// Gravity tractor mission duration in years.
    pub gt_duration_years: f64,
    /// Laser ablation power in kW.
    pub laser_power_kw: f64,
    /// Laser ablation mission duration in months.
    pub laser_duration_months: f64,
}

/// Payload type selection.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PayloadType {
    #[default]
    Kinetic,
    Nuclear,
    // Continuous deflection methods (Phase 6)
    IonBeam,
    GravityTractor,
    LaserAblation,
}

/// Propulsion technology level affecting interceptor speed.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum PropulsionLevel {
    /// Current chemical propulsion (~15 km/s average transfer speed).
    /// Similar to DART mission performance.
    #[default]
    Current,
    /// Advanced propulsion 3x faster (~45 km/s).
    /// Represents advanced ion propulsion or nuclear thermal.
    Advanced3x,
    /// Advanced propulsion 5x faster (~75 km/s).
    /// Represents futuristic propulsion concepts.
    Advanced5x,
    /// Advanced propulsion 10x faster (~150 km/s).
    /// Sci-fi level propulsion for dramatic scenarios.
    Advanced10x,
}

impl PropulsionLevel {
    /// Get the speed multiplier for this propulsion level.
    fn multiplier(&self) -> f64 {
        match self {
            PropulsionLevel::Current => 1.0,
            PropulsionLevel::Advanced3x => 3.0,
            PropulsionLevel::Advanced5x => 5.0,
            PropulsionLevel::Advanced10x => 10.0,
        }
    }

    /// Get the effective speed in m/s.
    fn speed(&self) -> f64 {
        BASE_INTERCEPTOR_SPEED * self.multiplier()
    }

    /// Get a description of this propulsion level.
    fn description(&self) -> &'static str {
        match self {
            PropulsionLevel::Current => "Chemical (current tech)",
            PropulsionLevel::Advanced3x => "Advanced (3× speed)",
            PropulsionLevel::Advanced5x => "Futuristic (5× speed)",
            PropulsionLevel::Advanced10x => "Experimental (10× speed)",
        }
    }

    /// Get the speed in km/s for display.
    fn speed_km_s(&self) -> f64 {
        self.speed() / 1000.0
    }
}

/// Direction selection mode.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum DirectionMode {
    /// Opposite to asteroid velocity (usually optimal).
    #[default]
    Retrograde,
    /// Same direction as asteroid velocity.
    Prograde,
    /// Perpendicular to orbit (radial).
    Radial,
    /// Custom angle.
    Custom,
}

impl Default for InterceptorLaunchState {
    fn default() -> Self {
        Self {
            open: false,
            payload_type: PayloadType::Kinetic,
            kinetic_mass: 560.0,      // DART mass
            kinetic_beta: 3.6,        // DART beta
            nuclear_yield: 100.0,     // 100 kt
            propulsion_level: PropulsionLevel::Current,
            direction_mode: DirectionMode::Retrograde,
            custom_angle: 0.0,
            // Continuous deflection defaults (Phase 6)
            ion_thrust_mn: 100.0,     // 100 mN
            ion_fuel_mass_kg: 500.0,  // 500 kg propellant
            ion_isp: 3500.0,          // Typical xenon ion engine
            gt_spacecraft_mass_kg: 20_000.0, // 20 tons
            gt_duration_years: 10.0,  // 10 years
            laser_power_kw: 100.0,    // 100 kW
            laser_duration_months: 12.0, // 1 year
        }
    }
}

impl InterceptorLaunchState {
    /// Check if the selected payload type is continuous (vs instant).
    fn is_continuous(&self) -> bool {
        matches!(
            self.payload_type,
            PayloadType::IonBeam | PayloadType::GravityTractor | PayloadType::LaserAblation
        )
    }

    /// Build the configured instant payload.
    /// Returns None for continuous payload types.
    fn build_payload(&self) -> Option<DeflectionPayload> {
        match self.payload_type {
            PayloadType::Kinetic => Some(DeflectionPayload::Kinetic {
                mass_kg: self.kinetic_mass,
                beta: self.kinetic_beta,
            }),
            PayloadType::Nuclear => Some(DeflectionPayload::Nuclear {
                yield_kt: self.nuclear_yield,
            }),
            PayloadType::IonBeam | PayloadType::GravityTractor | PayloadType::LaserAblation => None,
        }
    }

    /// Build the configured continuous payload.
    /// Returns None for instant payload types.
    fn build_continuous_payload(&self) -> Option<ContinuousPayload> {
        let direction = match self.direction_mode {
            DirectionMode::Retrograde => ThrustDirection::Retrograde,
            DirectionMode::Prograde => ThrustDirection::Prograde,
            DirectionMode::Radial => ThrustDirection::Radial,
            DirectionMode::Custom => {
                let rad = self.custom_angle.to_radians();
                ThrustDirection::Custom(DVec2::new(rad.cos(), rad.sin()))
            }
        };

        match self.payload_type {
            PayloadType::IonBeam => Some(ContinuousPayload::IonBeam {
                thrust_n: self.ion_thrust_mn / 1000.0, // Convert mN to N
                fuel_mass_kg: self.ion_fuel_mass_kg,
                specific_impulse: self.ion_isp,
                hover_distance_m: 200.0, // Fixed for now
                direction,
            }),
            PayloadType::GravityTractor => Some(ContinuousPayload::GravityTractor {
                spacecraft_mass_kg: self.gt_spacecraft_mass_kg,
                hover_distance_m: 200.0,
                mission_duration: self.gt_duration_years * 365.25 * 86400.0, // Years to seconds
                direction,
            }),
            PayloadType::LaserAblation => Some(ContinuousPayload::LaserAblation {
                power_kw: self.laser_power_kw,
                mission_duration: self.laser_duration_months * 30.44 * 86400.0, // Months to seconds
                efficiency: 0.8, // Fixed efficiency
                direction,
            }),
            PayloadType::Kinetic | PayloadType::Nuclear => None,
        }
    }

    /// Calculate direction vector based on mode and asteroid state.
    fn calculate_direction(&self, asteroid_vel: DVec2) -> DVec2 {
        match self.direction_mode {
            DirectionMode::Retrograde => -asteroid_vel.normalize_or_zero(),
            DirectionMode::Prograde => asteroid_vel.normalize_or_zero(),
            DirectionMode::Radial => {
                // Perpendicular to velocity (radially outward)
                let vel_norm = asteroid_vel.normalize_or_zero();
                DVec2::new(-vel_norm.y, vel_norm.x)
            }
            DirectionMode::Custom => {
                let rad = self.custom_angle.to_radians();
                DVec2::new(rad.cos(), rad.sin())
            }
        }
    }

    /// Calculate flight time in seconds based on distance and propulsion level.
    fn calculate_flight_time(&self, distance: f64) -> f64 {
        distance / self.propulsion_level.speed()
    }
}

/// System to render the interceptor launch window.
#[allow(clippy::too_many_arguments)]
pub fn interceptor_launch_system(
    mut contexts: EguiContexts,
    mut launch_state: ResMut<InterceptorLaunchState>,
    mut launch_events: EventWriter<LaunchInterceptorEvent>,
    mut continuous_launch_events: EventWriter<LaunchContinuousDeflectorEvent>,
    selected: Res<SelectedBody>,
    asteroids: Query<(Entity, &BodyState, Option<&TrajectoryPath>), With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
) {
    if !launch_state.open {
        return;
    }

    // Get selected asteroid
    let selected_asteroid = if let Some(SelectableBody::Asteroid(entity)) = selected.body {
        asteroids.get(entity).ok()
    } else {
        None
    };

    let ctx = contexts.ctx_mut();

    egui::Window::new("🎯 Launch Interceptor")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_CENTER, egui::vec2(-20.0, 0.0))
        .default_width(320.0)
        .show(ctx, |ui| {
            if selected_asteroid.is_none() {
                ui.label("⚠️ No asteroid selected");
                ui.label("Select an asteroid to launch an interceptor.");
                if ui.button("Close").clicked() {
                    launch_state.open = false;
                }
                return;
            }

            let (entity, asteroid_state, trajectory) = selected_asteroid.unwrap();

            // Calculate distance from Earth to asteroid
            let earth_pos = ephemeris
                .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
                .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
            let distance = (asteroid_state.pos - earth_pos).length();
            let distance_au = distance / AU_TO_METERS;

            // Calculate flight time
            let flight_time_seconds = launch_state.calculate_flight_time(distance);
            let flight_time_days = flight_time_seconds / SECONDS_PER_DAY;

            // Check for collision warning
            let time_to_collision = trajectory.and_then(|t| {
                if let TrajectoryOutcome::Collision { time_to_impact, .. } = &t.outcome {
                    Some(*time_to_impact)
                } else {
                    None
                }
            });
            let will_arrive_late = time_to_collision
                .map(|ttc| flight_time_seconds > ttc)
                .unwrap_or(false);

            // Payload type selection - Instant methods
            ui.heading("Instant Deflection");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut launch_state.payload_type, PayloadType::Kinetic, "Kinetic");
                ui.selectable_value(&mut launch_state.payload_type, PayloadType::Nuclear, "Nuclear");
            });

            ui.add_space(4.0);

            // Payload type selection - Continuous methods
            ui.heading("Continuous Deflection");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut launch_state.payload_type, PayloadType::IonBeam, "Ion Beam");
                ui.selectable_value(&mut launch_state.payload_type, PayloadType::GravityTractor, "Gravity");
                ui.selectable_value(&mut launch_state.payload_type, PayloadType::LaserAblation, "Laser");
            });

            ui.add_space(8.0);

            // Payload parameters
            ui.heading("Parameters");
            match launch_state.payload_type {
                PayloadType::Kinetic => {
                    ui.horizontal(|ui| {
                        ui.label("Mass (kg):");
                        ui.add(egui::Slider::new(&mut launch_state.kinetic_mass, 100.0..=2000.0).suffix(" kg"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Beta factor:");
                        ui.add(egui::Slider::new(&mut launch_state.kinetic_beta, 1.0..=5.0).step_by(0.1));
                    });
                    ui.label(egui::RichText::new("β = ejecta momentum enhancement (DART: 3.6)").weak().small());
                }
                PayloadType::Nuclear => {
                    ui.horizontal(|ui| {
                        ui.label("Yield:");
                        ui.add(egui::Slider::new(&mut launch_state.nuclear_yield, 1.0..=1000.0)
                            .logarithmic(true)
                            .suffix(" kt"));
                    });
                    ui.label(egui::RichText::new("Standoff detonation vaporizes surface material").weak().small());
                }
                PayloadType::IonBeam => {
                    ui.horizontal(|ui| {
                        ui.label("Thrust (mN):");
                        ui.add(egui::Slider::new(&mut launch_state.ion_thrust_mn, 10.0..=1000.0)
                            .logarithmic(true)
                            .suffix(" mN"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Fuel mass:");
                        ui.add(egui::Slider::new(&mut launch_state.ion_fuel_mass_kg, 100.0..=2000.0).suffix(" kg"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Isp:");
                        ui.add(egui::Slider::new(&mut launch_state.ion_isp, 2000.0..=5000.0).suffix(" s"));
                    });
                    ui.label(egui::RichText::new("Spacecraft hovers near asteroid, ion exhaust pushes it").weak().small());
                }
                PayloadType::GravityTractor => {
                    ui.horizontal(|ui| {
                        ui.label("Mass:");
                        ui.add(egui::Slider::new(&mut launch_state.gt_spacecraft_mass_kg, 5000.0..=50000.0)
                            .suffix(" kg"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Duration:");
                        ui.add(egui::Slider::new(&mut launch_state.gt_duration_years, 1.0..=20.0).suffix(" years"));
                    });
                    ui.label(egui::RichText::new("Spacecraft mass gravitationally pulls asteroid").weak().small());
                }
                PayloadType::LaserAblation => {
                    ui.horizontal(|ui| {
                        ui.label("Power:");
                        ui.add(egui::Slider::new(&mut launch_state.laser_power_kw, 10.0..=1000.0)
                            .logarithmic(true)
                            .suffix(" kW"));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Duration:");
                        ui.add(egui::Slider::new(&mut launch_state.laser_duration_months, 1.0..=36.0).suffix(" months"));
                    });
                    ui.label(egui::RichText::new("Vaporizes asteroid surface, creating thrust plume").weak().small());
                }
            }

            ui.add_space(8.0);

            // Propulsion technology selection
            ui.heading("Propulsion Technology");
            ui.vertical(|ui| {
                ui.radio_value(
                    &mut launch_state.propulsion_level,
                    PropulsionLevel::Current,
                    format!("🚀 {} (~{:.0} km/s)",
                        PropulsionLevel::Current.description(),
                        PropulsionLevel::Current.speed_km_s()
                    ),
                );
                ui.radio_value(
                    &mut launch_state.propulsion_level,
                    PropulsionLevel::Advanced3x,
                    format!("⚡ {} (~{:.0} km/s)",
                        PropulsionLevel::Advanced3x.description(),
                        PropulsionLevel::Advanced3x.speed_km_s()
                    ),
                );
                ui.radio_value(
                    &mut launch_state.propulsion_level,
                    PropulsionLevel::Advanced5x,
                    format!("🔬 {} (~{:.0} km/s)",
                        PropulsionLevel::Advanced5x.description(),
                        PropulsionLevel::Advanced5x.speed_km_s()
                    ),
                );
                ui.radio_value(
                    &mut launch_state.propulsion_level,
                    PropulsionLevel::Advanced10x,
                    format!("🛸 {} (~{:.0} km/s)",
                        PropulsionLevel::Advanced10x.description(),
                        PropulsionLevel::Advanced10x.speed_km_s()
                    ),
                );
            });

            ui.add_space(8.0);

            // Direction selection
            ui.heading("Deflection Direction");
            ui.vertical(|ui| {
                ui.radio_value(&mut launch_state.direction_mode, DirectionMode::Retrograde, "Retrograde (optimal)");
                ui.radio_value(&mut launch_state.direction_mode, DirectionMode::Prograde, "Prograde");
                ui.radio_value(&mut launch_state.direction_mode, DirectionMode::Radial, "Radial (perpendicular)");
                ui.radio_value(&mut launch_state.direction_mode, DirectionMode::Custom, "Custom angle");
            });

            if launch_state.direction_mode == DirectionMode::Custom {
                ui.horizontal(|ui| {
                    ui.label("Angle:");
                    ui.add(egui::Slider::new(&mut launch_state.custom_angle, 0.0..=360.0).suffix("°"));
                });
            }

            ui.add_space(12.0);

            // Mission Details
            ui.separator();
            ui.heading("Mission Details");

            // Distance info
            ui.label(format!("Distance to target: {:.3} AU ({:.1} million km)",
                distance_au,
                distance / 1e9
            ));

            // Flight time with warning color if late
            let flight_time_text = format!("Estimated flight time: {:.1} days", flight_time_days);
            if will_arrive_late {
                ui.label(egui::RichText::new(flight_time_text).color(egui::Color32::from_rgb(255, 100, 100)));
                ui.label(egui::RichText::new("⚠️ Interceptor will arrive AFTER collision!")
                    .color(egui::Color32::from_rgb(255, 100, 100))
                    .small());
            } else {
                ui.label(flight_time_text);
            }

            // Show time to collision if known
            if let Some(ttc) = time_to_collision {
                let ttc_days = ttc / SECONDS_PER_DAY;
                ui.label(format!("Time to collision: {:.1} days", ttc_days));
                if !will_arrive_late {
                    let margin = ttc_days - flight_time_days;
                    ui.label(egui::RichText::new(format!("Arrival margin: {:.1} days before impact", margin))
                        .color(egui::Color32::from_rgb(100, 200, 100)));
                }
            }

            ui.add_space(8.0);

            // Preview
            ui.separator();
            ui.heading("Deflection Preview");

            let direction = launch_state.calculate_direction(asteroid_state.vel);
            let is_continuous = launch_state.is_continuous();

            ui.label(format!("Asteroid mass: {:.2e} kg", asteroid_state.mass));
            ui.label(format!(
                "Direction: ({:.2}, {:.2})",
                direction.x, direction.y
            ));

            // Show estimated delta-v based on payload type
            if let Some(payload) = launch_state.build_payload() {
                let estimated_dv = payload.estimate_delta_v(asteroid_state.mass);
                ui.label(format!("Payload: {}", payload.description()));
                ui.label(format!(
                    "Estimated Δv: {:.4} mm/s",
                    estimated_dv * 1000.0
                ));
            } else if let Some(payload) = launch_state.build_continuous_payload() {
                // Calculate estimated total delta-v for continuous methods
                let solar_distance_au = asteroid_state.pos.length() / AU_TO_METERS;
                let estimated_dv = payload.estimate_total_delta_v(asteroid_state.mass, solar_distance_au);
                ui.label(format!("Method: {}", payload.name()));
                ui.label(format!(
                    "Est. total Δv: {:.4} mm/s",
                    estimated_dv * 1000.0
                ));
                ui.label(egui::RichText::new("(applied over mission duration)").weak().small());
            }

            ui.add_space(12.0);

            // Launch button - different handling for instant vs continuous
            ui.horizontal(|ui| {
                // For continuous methods, we don't check arrival time (they need time to work)
                let launch_enabled = is_continuous || !will_arrive_late;
                let button_text = if is_continuous {
                    "🛰️ Deploy"
                } else {
                    "🚀 Launch"
                };

                let launch_button = ui.add_enabled(
                    launch_enabled,
                    egui::Button::new(egui::RichText::new(button_text).size(16.0))
                );

                if launch_button.clicked() {
                    if let Some(payload) = launch_state.build_payload() {
                        // Launch instant interceptor
                        launch_events.send(LaunchInterceptorEvent {
                            target: entity,
                            payload,
                            direction: Some(direction),
                            flight_time: Some(flight_time_seconds),
                        });
                    } else if let Some(payload) = launch_state.build_continuous_payload() {
                        // Launch continuous deflector
                        continuous_launch_events.send(LaunchContinuousDeflectorEvent {
                            target: entity,
                            payload,
                            flight_time: flight_time_seconds,
                        });
                    }
                    launch_state.open = false;
                }

                if ui.button("Cancel").clicked() {
                    launch_state.open = false;
                }
            });

            if !is_continuous && will_arrive_late {
                ui.label(egui::RichText::new("Select faster propulsion to launch")
                    .color(egui::Color32::from_rgb(255, 150, 100))
                    .small());
            }
        });
}
