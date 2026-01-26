//! Continuous deflection methods for asteroid deflection.
//!
//! Unlike instant deflection (kinetic impactor, nuclear), continuous methods
//! apply small forces over extended periods. This requires integration into
//! the physics loop rather than one-time delta-v application.
//!
//! # Methods
//!
//! - **Ion Beam Shepherd**: Direct momentum transfer from ion exhaust
//! - **Gravity Tractor**: Gravitational pull from hovering spacecraft
//! - **Laser Ablation**: Thrust from vaporized surface material

pub mod payload;
pub mod thrust;

use bevy::math::DVec2;
use bevy::prelude::*;

pub use payload::ContinuousPayload;
pub use thrust::ThrustDirection;

use crate::asteroid::Asteroid;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::interceptor::{TRANSFER_ARC_POINTS, generate_transfer_arc, predict_asteroid_at_time};
use crate::lambert::solve_lambert_auto;
use crate::physics::IntegratorStates;
use crate::prediction::{PredictionState, mark_prediction_dirty};
use crate::types::{AU_TO_METERS, BodyState, GM_SUN, SimulationTime};

use self::thrust::{
    compute_thrust_direction, ion_beam_acceleration, ion_fuel_consumption_rate,
    laser_ablation_acceleration, solar_sail_acceleration,
};

/// State of a continuous deflection mission.
#[derive(Clone, Debug, PartialEq)]
pub enum ContinuousDeflectorState {
    /// Spacecraft is traveling to the asteroid.
    EnRoute {
        /// Arrival time in J2000 seconds.
        arrival_time: f64,
    },
    /// Spacecraft is actively deflecting the asteroid.
    Operating {
        /// Time when operation started (J2000 seconds).
        started_at: f64,
        /// Fuel consumed so far (kg) - only for ion beam.
        fuel_consumed: f64,
        /// Delta-v accumulated so far (m/s).
        accumulated_delta_v: f64,
    },
    /// Mission ended due to fuel depletion.
    FuelDepleted {
        /// Time when fuel ran out.
        ended_at: f64,
        /// Total delta-v delivered.
        total_delta_v: f64,
    },
    /// Mission completed (duration-based methods).
    Complete {
        /// Time when mission ended.
        ended_at: f64,
        /// Total delta-v delivered.
        total_delta_v: f64,
    },
    /// Mission was cancelled.
    Cancelled,
}

impl Default for ContinuousDeflectorState {
    fn default() -> Self {
        ContinuousDeflectorState::EnRoute { arrival_time: 0.0 }
    }
}

/// Component for a continuous deflection spacecraft.
#[derive(Component, Clone, Debug)]
pub struct ContinuousDeflector {
    /// Target asteroid entity.
    pub target: Entity,
    /// Deflection payload configuration.
    pub payload: ContinuousPayload,
    /// Launch time (J2000 seconds).
    pub launch_time: f64,
    /// Launch position (Earth position at launch).
    pub launch_position: DVec2,
    /// Predicted arrival position (asteroid position at arrival time).
    pub arrival_position: DVec2,
    /// Transfer orbit arc points for curved trajectory visualization.
    /// If empty, falls back to linear interpolation.
    pub transfer_arc: Vec<DVec2>,
    /// Departure velocity from Lambert solution (for display).
    pub departure_velocity: DVec2,
    /// Current mission state.
    pub state: ContinuousDeflectorState,
}

impl ContinuousDeflector {
    /// Check if this deflector is currently applying thrust.
    pub fn is_operating(&self) -> bool {
        matches!(self.state, ContinuousDeflectorState::Operating { .. })
    }

    /// Check if this deflector's mission is complete (for any reason).
    pub fn is_finished(&self) -> bool {
        matches!(
            self.state,
            ContinuousDeflectorState::FuelDepleted { .. }
                | ContinuousDeflectorState::Complete { .. }
                | ContinuousDeflectorState::Cancelled
        )
    }

    /// Get accumulated delta-v if operating or finished.
    pub fn accumulated_delta_v(&self) -> f64 {
        match &self.state {
            ContinuousDeflectorState::Operating {
                accumulated_delta_v,
                ..
            } => *accumulated_delta_v,
            ContinuousDeflectorState::FuelDepleted { total_delta_v, .. } => *total_delta_v,
            ContinuousDeflectorState::Complete { total_delta_v, .. } => *total_delta_v,
            _ => 0.0,
        }
    }

    /// Get remaining fuel fraction (0.0 - 1.0) for fuel-based methods.
    pub fn fuel_fraction(&self) -> Option<f64> {
        if let ContinuousDeflectorState::Operating { fuel_consumed, .. } = &self.state
            && let Some(initial) = self.payload.initial_fuel()
            && initial > 0.0
        {
            return Some(1.0 - (fuel_consumed / initial).min(1.0));
        }
        None
    }
}

/// Registry for tracking all continuous deflectors.
#[derive(Resource, Default)]
pub struct ContinuousDeflectorRegistry {
    /// Total number of continuous deflectors launched.
    pub total_launched: u32,
}

/// Event to launch a new continuous deflector.
#[derive(Message)]
pub struct LaunchContinuousDeflectorEvent {
    /// Target asteroid entity.
    pub target: Entity,
    /// Payload configuration.
    pub payload: ContinuousPayload,
    /// Flight time in seconds (time to reach asteroid).
    pub flight_time: f64,
}

/// Plugin for continuous deflection systems.
pub struct ContinuousPlugin;

impl Plugin for ContinuousPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ContinuousDeflectorRegistry>()
            .init_resource::<Messages<LaunchContinuousDeflectorEvent>>()
            .add_systems(Update, handle_launch_event)
            .add_systems(
                Update,
                update_continuous_deflectors.after(handle_launch_event),
            );
    }
}

/// Handle launch events to spawn new continuous deflectors.
///
/// Uses Lambert solver to compute realistic transfer orbit from Earth to
/// the predicted asteroid position at arrival time.
fn handle_launch_event(
    mut commands: Commands,
    mut events: MessageReader<LaunchContinuousDeflectorEvent>,
    mut registry: ResMut<ContinuousDeflectorRegistry>,
    sim_time: Res<SimulationTime>,
    ephemeris: Res<Ephemeris>,
    asteroids: Query<&BodyState, With<Asteroid>>,
) {
    for event in events.read() {
        // Verify target exists
        let Ok(asteroid_state) = asteroids.get(event.target) else {
            warn!("Continuous deflector launch failed: target asteroid not found");
            continue;
        };

        // Get Earth position for launch point
        let earth_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
            .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

        let flight_time = event.flight_time;
        let arrival_time = sim_time.current + flight_time;

        // Predict asteroid position at arrival time
        let (arrival_position, _arrival_vel) =
            predict_asteroid_at_time(asteroid_state, sim_time.current, arrival_time, &ephemeris);

        // For instant deployment (flight_time = 0), skip Lambert calculation
        let (transfer_arc, departure_velocity) = if flight_time > 0.0 {
            // Try to solve Lambert's problem for transfer orbit
            match solve_lambert_auto(earth_pos, arrival_position, flight_time, GM_SUN) {
                Some(solution) => {
                    // Generate arc points for visualization
                    let arc = generate_transfer_arc(
                        earth_pos,
                        solution.v1,
                        flight_time,
                        TRANSFER_ARC_POINTS,
                    );
                    (arc, solution.v1)
                }
                None => {
                    // Fallback: no curved arc, just linear interpolation
                    warn!(
                        "Lambert solver did not converge for continuous deflector, using linear trajectory"
                    );
                    (Vec::new(), DVec2::ZERO)
                }
            }
        } else {
            // Instant deployment - no transit
            (Vec::new(), DVec2::ZERO)
        };

        // Spawn the deflector entity
        commands.spawn(ContinuousDeflector {
            target: event.target,
            payload: event.payload.clone(),
            launch_time: sim_time.current,
            launch_position: earth_pos,
            arrival_position,
            transfer_arc,
            departure_velocity,
            state: ContinuousDeflectorState::EnRoute { arrival_time },
        });

        registry.total_launched += 1;

        if flight_time > 0.0 {
            info!(
                "Launched {} continuous deflector, arrival in {:.1} days",
                event.payload.name(),
                flight_time / 86400.0
            );
        } else {
            info!(
                "Deployed {} (instant effect from Earth-based platform)",
                event.payload.name()
            );
        }
    }
}

/// Update continuous deflectors - state transitions and fuel consumption.
#[allow(clippy::too_many_arguments)]
fn update_continuous_deflectors(
    mut commands: Commands,
    mut deflectors: Query<(Entity, &mut ContinuousDeflector)>,
    asteroids: Query<&BodyState, With<Asteroid>>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    sim_time: Res<SimulationTime>,
) {
    let current_time = sim_time.current;

    for (entity, mut deflector) in deflectors.iter_mut() {
        match &deflector.state {
            ContinuousDeflectorState::EnRoute { arrival_time } => {
                if current_time >= *arrival_time {
                    // Transition to operating
                    deflector.state = ContinuousDeflectorState::Operating {
                        started_at: current_time,
                        fuel_consumed: 0.0,
                        accumulated_delta_v: 0.0,
                    };

                    // Reset integrator for new forces
                    integrator_states.remove(deflector.target);
                    mark_prediction_dirty(&mut prediction_state);

                    info!(
                        "{} deflector arrived and began operating",
                        deflector.payload.name()
                    );
                }
            }
            ContinuousDeflectorState::Operating {
                started_at,
                fuel_consumed,
                accumulated_delta_v,
            } => {
                let started_at = *started_at;
                let fuel_consumed = *fuel_consumed;
                let accumulated_delta_v = *accumulated_delta_v;

                // Check if target still exists
                if asteroids.get(deflector.target).is_err() {
                    deflector.state = ContinuousDeflectorState::Cancelled;
                    commands.entity(entity).despawn();
                    continue;
                }

                // Check for mission completion based on payload type
                let should_complete = match &deflector.payload {
                    ContinuousPayload::IonBeam { fuel_mass_kg, .. } => {
                        fuel_consumed >= *fuel_mass_kg
                    }
                    ContinuousPayload::LaserAblation {
                        mission_duration, ..
                    } => (current_time - started_at) >= *mission_duration,
                    ContinuousPayload::SolarSail {
                        mission_duration, ..
                    } => (current_time - started_at) >= *mission_duration,
                };

                if should_complete {
                    let new_state =
                        if matches!(deflector.payload, ContinuousPayload::IonBeam { .. }) {
                            ContinuousDeflectorState::FuelDepleted {
                                ended_at: current_time,
                                total_delta_v: accumulated_delta_v,
                            }
                        } else {
                            ContinuousDeflectorState::Complete {
                                ended_at: current_time,
                                total_delta_v: accumulated_delta_v,
                            }
                        };

                    deflector.state = new_state;

                    // Reset integrator since forces changed
                    integrator_states.remove(deflector.target);
                    mark_prediction_dirty(&mut prediction_state);

                    info!(
                        "{} deflector mission complete. Total Δv: {:.4} mm/s",
                        deflector.payload.name(),
                        accumulated_delta_v * 1000.0
                    );
                }
            }
            ContinuousDeflectorState::FuelDepleted { .. }
            | ContinuousDeflectorState::Complete { .. }
            | ContinuousDeflectorState::Cancelled => {
                // Finished deflectors stay around for UI display, can be cleaned up later
            }
        }
    }
}

/// Compute the total continuous thrust acceleration for an asteroid.
///
/// This function aggregates thrust from all active continuous deflectors
/// targeting the given asteroid.
///
/// # Arguments
/// * `target_entity` - The asteroid entity
/// * `asteroid_pos` - Current asteroid position (m)
/// * `asteroid_vel` - Current asteroid velocity (m/s)
/// * `asteroid_mass` - Asteroid mass (kg)
/// * `sim_time` - Current simulation time
/// * `deflectors` - Query for all continuous deflectors
///
/// # Returns
/// Acceleration vector in m/s² from all active deflectors
pub fn compute_continuous_thrust(
    target_entity: Entity,
    asteroid_pos: DVec2,
    asteroid_vel: DVec2,
    asteroid_mass: f64,
    _sim_time: f64,
    deflectors: &[(Entity, &ContinuousDeflector)],
) -> DVec2 {
    let mut total_acc = DVec2::ZERO;

    for (_deflector_entity, deflector) in deflectors.iter() {
        // Only consider deflectors targeting this asteroid
        if deflector.target != target_entity {
            continue;
        }

        // Only operating deflectors apply thrust
        if !deflector.is_operating() {
            continue;
        }

        // Compute acceleration magnitude based on payload type
        let acc_magnitude = match &deflector.payload {
            ContinuousPayload::IonBeam { thrust_n, .. } => {
                ion_beam_acceleration(*thrust_n, asteroid_mass)
            }
            ContinuousPayload::LaserAblation {
                power_kw,
                efficiency,
                ..
            } => {
                // Calculate solar distance in AU for efficiency calculation
                let solar_distance_au = asteroid_pos.length() / AU_TO_METERS;
                laser_ablation_acceleration(power_kw * efficiency, solar_distance_au, asteroid_mass)
            }
            ContinuousPayload::SolarSail {
                sail_area_m2,
                reflectivity,
                ..
            } => {
                let solar_distance_au = asteroid_pos.length() / AU_TO_METERS;
                solar_sail_acceleration(
                    sail_area_m2 * reflectivity,
                    solar_distance_au,
                    asteroid_mass,
                )
            }
        };

        // Compute thrust direction
        let direction =
            compute_thrust_direction(asteroid_vel, asteroid_pos, deflector.payload.direction());

        // Add to total acceleration
        total_acc += direction * acc_magnitude;
    }

    total_acc
}

/// Update fuel consumption and accumulated delta-v for active deflectors.
///
/// This should be called after each physics step to track mission progress.
pub fn update_deflector_progress(
    deflector: &mut ContinuousDeflector,
    asteroid_mass: f64,
    asteroid_pos: DVec2,
    dt: f64,
) {
    if let ContinuousDeflectorState::Operating {
        fuel_consumed,
        accumulated_delta_v,
        ..
    } = &mut deflector.state
    {
        let acc_magnitude = match &deflector.payload {
            ContinuousPayload::IonBeam {
                thrust_n,
                specific_impulse,
                ..
            } => {
                // Update fuel consumption
                let mdot = ion_fuel_consumption_rate(*thrust_n, *specific_impulse);
                *fuel_consumed += mdot * dt;

                ion_beam_acceleration(*thrust_n, asteroid_mass)
            }
            ContinuousPayload::LaserAblation {
                power_kw,
                efficiency,
                ..
            } => {
                let solar_distance_au = asteroid_pos.length() / AU_TO_METERS;
                laser_ablation_acceleration(power_kw * efficiency, solar_distance_au, asteroid_mass)
            }
            ContinuousPayload::SolarSail {
                sail_area_m2,
                reflectivity,
                ..
            } => {
                let solar_distance_au = asteroid_pos.length() / AU_TO_METERS;
                solar_sail_acceleration(
                    sail_area_m2 * reflectivity,
                    solar_distance_au,
                    asteroid_mass,
                )
            }
        };

        // Update accumulated delta-v
        *accumulated_delta_v += acc_magnitude * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deflector_state_default() {
        let state = ContinuousDeflectorState::default();
        assert!(matches!(state, ContinuousDeflectorState::EnRoute { .. }));
    }

    #[test]
    fn test_deflector_is_operating() {
        let mut deflector = ContinuousDeflector {
            target: Entity::PLACEHOLDER,
            payload: ContinuousPayload::ion_beam_default(),
            launch_time: 0.0,
            launch_position: DVec2::ZERO,
            arrival_position: DVec2::ZERO,
            transfer_arc: Vec::new(),
            departure_velocity: DVec2::ZERO,
            state: ContinuousDeflectorState::EnRoute {
                arrival_time: 100.0,
            },
        };

        assert!(!deflector.is_operating());

        deflector.state = ContinuousDeflectorState::Operating {
            started_at: 100.0,
            fuel_consumed: 0.0,
            accumulated_delta_v: 0.0,
        };

        assert!(deflector.is_operating());
    }

    #[test]
    fn test_deflector_is_finished() {
        let mut deflector = ContinuousDeflector {
            target: Entity::PLACEHOLDER,
            payload: ContinuousPayload::ion_beam_default(),
            launch_time: 0.0,
            launch_position: DVec2::ZERO,
            arrival_position: DVec2::ZERO,
            transfer_arc: Vec::new(),
            departure_velocity: DVec2::ZERO,
            state: ContinuousDeflectorState::Operating {
                started_at: 100.0,
                fuel_consumed: 0.0,
                accumulated_delta_v: 0.0,
            },
        };

        assert!(!deflector.is_finished());

        deflector.state = ContinuousDeflectorState::FuelDepleted {
            ended_at: 200.0,
            total_delta_v: 0.001,
        };

        assert!(deflector.is_finished());
    }

    #[test]
    fn test_fuel_fraction() {
        let deflector = ContinuousDeflector {
            target: Entity::PLACEHOLDER,
            payload: ContinuousPayload::IonBeam {
                thrust_n: 0.1,
                fuel_mass_kg: 100.0,
                specific_impulse: 3000.0,
                hover_distance_m: 200.0,
                direction: ThrustDirection::Retrograde,
            },
            launch_time: 0.0,
            launch_position: DVec2::ZERO,
            arrival_position: DVec2::ZERO,
            transfer_arc: Vec::new(),
            departure_velocity: DVec2::ZERO,
            state: ContinuousDeflectorState::Operating {
                started_at: 100.0,
                fuel_consumed: 25.0, // 25% consumed
                accumulated_delta_v: 0.0,
            },
        };

        let fraction = deflector.fuel_fraction().unwrap();
        assert!((fraction - 0.75).abs() < 1e-10);
    }

    #[test]
    fn test_compute_continuous_thrust_single() {
        let deflector = ContinuousDeflector {
            target: Entity::PLACEHOLDER,
            payload: ContinuousPayload::IonBeam {
                thrust_n: 0.1, // 100 mN
                fuel_mass_kg: 100.0,
                specific_impulse: 3000.0,
                hover_distance_m: 200.0,
                direction: ThrustDirection::Retrograde,
            },
            launch_time: 0.0,
            launch_position: DVec2::ZERO,
            arrival_position: DVec2::ZERO,
            transfer_arc: Vec::new(),
            departure_velocity: DVec2::ZERO,
            state: ContinuousDeflectorState::Operating {
                started_at: 100.0,
                fuel_consumed: 0.0,
                accumulated_delta_v: 0.0,
            },
        };

        let target = Entity::PLACEHOLDER;
        let pos = DVec2::new(AU_TO_METERS, 0.0);
        let vel = DVec2::new(0.0, 30000.0); // ~30 km/s circular velocity
        let mass = 1e10; // 10 billion kg

        let deflectors: Vec<(Entity, &ContinuousDeflector)> =
            vec![(Entity::PLACEHOLDER, &deflector)];
        let acc = compute_continuous_thrust(target, pos, vel, mass, 0.0, &deflectors);

        // Acceleration should be opposite to velocity (retrograde)
        assert!(acc.x.abs() < 1e-20);
        assert!(acc.y < 0.0); // Pointing opposite to velocity

        // Check magnitude: F/m = 0.1 / 1e10 = 1e-11 m/s²
        assert!((acc.length() - 1e-11).abs() < 1e-20);
    }

    #[test]
    fn test_compute_continuous_thrust_non_operating() {
        let deflector = ContinuousDeflector {
            target: Entity::PLACEHOLDER,
            payload: ContinuousPayload::ion_beam_default(),
            launch_time: 0.0,
            launch_position: DVec2::ZERO,
            arrival_position: DVec2::ZERO,
            transfer_arc: Vec::new(),
            departure_velocity: DVec2::ZERO,
            state: ContinuousDeflectorState::EnRoute {
                arrival_time: 100.0,
            },
        };

        let target = Entity::PLACEHOLDER;
        let pos = DVec2::new(AU_TO_METERS, 0.0);
        let vel = DVec2::new(0.0, 30000.0);
        let mass = 1e10;

        let deflectors: Vec<(Entity, &ContinuousDeflector)> =
            vec![(Entity::PLACEHOLDER, &deflector)];
        let acc = compute_continuous_thrust(target, pos, vel, mass, 0.0, &deflectors);

        // Non-operating deflector should not contribute thrust
        assert_eq!(acc, DVec2::ZERO);
    }
}
