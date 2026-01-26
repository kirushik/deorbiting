//! Interceptor system for deflecting asteroids.
//!
//! Provides mechanics for launching interceptors from Earth to deflect asteroids:
//! - Kinetic impactors (DART-style)
//! - Nuclear standoff detonations
//! - Nuclear splitting (Armageddon style)
//!
//! Interceptors travel on simplified trajectories and apply delta-v on arrival.

pub mod payload;

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::asteroid::{Asteroid, AsteroidCounter, AsteroidName, spawn_asteroid};
use crate::camera::RENDER_SCALE;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::lambert::solve_lambert_auto;
use crate::physics::{IntegratorStates, compute_acceleration};
use crate::prediction::{PredictionState, TrajectoryPath, mark_prediction_dirty};
use crate::types::{AU_TO_METERS, BodyState, GM_SUN, SimulationTime};

pub use payload::DeflectionPayload;

/// Number of points to generate for transfer orbit visualization.
pub const TRANSFER_ARC_POINTS: usize = 50;

/// State of an interceptor mission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InterceptorState {
    /// Interceptor is traveling to target.
    InFlight,
    /// Interceptor has arrived and applied delta-v.
    Arrived,
    /// Mission was cancelled.
    Cancelled,
}

/// Component for an interceptor spacecraft.
#[derive(Component, Clone, Debug)]
pub struct Interceptor {
    /// Unique identifier for this interceptor (for color differentiation).
    pub id: u32,
    /// Target asteroid entity.
    pub target: Entity,
    /// Deflection payload configuration.
    pub payload: DeflectionPayload,
    /// Launch time (seconds since J2000).
    pub launch_time: f64,
    /// Expected arrival time (seconds since J2000).
    pub arrival_time: f64,
    /// Launch position (Earth position at launch).
    pub launch_position: DVec2,
    /// Predicted arrival position (asteroid position at arrival time).
    pub arrival_position: DVec2,
    /// Deflection direction (unit vector).
    pub deflection_direction: DVec2,
    /// Current state.
    pub state: InterceptorState,
    /// Transfer orbit arc points for curved trajectory visualization.
    /// If empty, falls back to linear interpolation.
    pub transfer_arc: Vec<DVec2>,
    /// Departure velocity from Lambert solution (for display).
    pub departure_velocity: DVec2,
}

/// Resource for tracking active interceptors.
#[derive(Resource, Default)]
pub struct InterceptorRegistry {
    /// Count of launched interceptors.
    pub total_launched: u32,
}

/// Event to launch a new interceptor.
#[derive(Message)]
pub struct LaunchInterceptorEvent {
    /// Target asteroid entity.
    pub target: Entity,
    /// Payload type.
    pub payload: DeflectionPayload,
    /// Deflection direction (optional, defaults to retrograde).
    pub direction: Option<DVec2>,
    /// Flight time in seconds (optional, defaults to 90 days).
    pub flight_time: Option<f64>,
}

/// Event to split an asteroid into two fragments.
#[derive(Message)]
pub struct SplitAsteroidEvent {
    /// The asteroid entity to split.
    pub target: Entity,
    /// Original asteroid position.
    pub position: DVec2,
    /// Original asteroid velocity.
    pub velocity: DVec2,
    /// Original asteroid mass.
    pub mass: f64,
    /// Original asteroid name.
    pub original_name: String,
    /// Nuclear yield used for splitting (kt).
    pub yield_kt: f64,
    /// Mass ratio for split (fraction for first fragment).
    pub split_ratio: f64,
    /// Direction of deflection (fragments separate perpendicular to this).
    pub deflection_direction: DVec2,
}

/// Plugin for interceptor management.
pub struct InterceptorPlugin;

impl Plugin for InterceptorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InterceptorRegistry>()
            .init_resource::<Messages<LaunchInterceptorEvent>>()
            .init_resource::<Messages<SplitAsteroidEvent>>()
            .add_systems(
                Update,
                (
                    handle_launch_event,
                    update_interceptors,
                    handle_asteroid_splitting,
                    draw_interceptor_trajectories,
                ),
            );
    }
}

/// Predict asteroid position at a future time using simple Verlet integration.
///
/// This is a lightweight prediction that doesn't store trajectory points,
/// just returns the final position and velocity.
pub fn predict_asteroid_at_time(
    initial_state: &BodyState,
    start_time: f64,
    target_time: f64,
    ephemeris: &Ephemeris,
) -> (DVec2, DVec2) {
    let dt_max = 3600.0; // 1 hour max timestep
    let mut pos = initial_state.pos;
    let mut vel = initial_state.vel;
    let mut t = start_time;

    while t < target_time {
        let dt = (target_time - t).min(dt_max);

        // Simple Velocity Verlet
        let acc1 = compute_acceleration(pos, t, ephemeris);
        let half_vel = vel + acc1 * dt * 0.5;
        pos = pos + half_vel * dt;
        let acc2 = compute_acceleration(pos, t + dt, ephemeris);
        vel = half_vel + acc2 * dt * 0.5;
        t += dt;
    }

    (pos, vel)
}

/// Result of asteroid position prediction with collision detection.
#[derive(Debug, Clone)]
pub struct AsteroidPredictionResult {
    /// Final position in meters from solar system barycenter.
    pub pos: DVec2,
    /// Final velocity in m/s.
    pub vel: DVec2,
    /// If collision was detected: (body_id, collision_time_seconds).
    pub collision: Option<(CelestialBodyId, f64)>,
}

/// Predict asteroid position at a future time, detecting collisions.
///
/// Like `predict_asteroid_at_time`, but also checks for collision at each
/// timestep. If collision is detected, returns early with collision info.
/// This is critical for interceptor targeting - we must not aim at positions
/// the asteroid will never reach.
fn predict_asteroid_at_time_with_collision(
    initial_state: &BodyState,
    start_time: f64,
    target_time: f64,
    ephemeris: &Ephemeris,
) -> AsteroidPredictionResult {
    let dt_max = 3600.0; // 1 hour max timestep
    let mut pos = initial_state.pos;
    let mut vel = initial_state.vel;
    let mut t = start_time;

    while t < target_time {
        let dt = (target_time - t).min(dt_max);

        // Simple Velocity Verlet
        let acc1 = compute_acceleration(pos, t, ephemeris);
        let half_vel = vel + acc1 * dt * 0.5;
        pos = pos + half_vel * dt;
        let acc2 = compute_acceleration(pos, t + dt, ephemeris);
        vel = half_vel + acc2 * dt * 0.5;
        t += dt;

        // Check for collision AFTER position update
        if let Some(body_id) = ephemeris.check_collision(pos, t) {
            return AsteroidPredictionResult {
                pos,
                vel,
                collision: Some((body_id, t)),
            };
        }
    }

    AsteroidPredictionResult {
        pos,
        vel,
        collision: None,
    }
}

/// Generate transfer orbit arc points using Kepler propagation from Lambert solution.
pub fn generate_transfer_arc(
    launch_pos: DVec2,
    departure_vel: DVec2,
    tof: f64,
    num_points: usize,
) -> Vec<DVec2> {
    let mut points = Vec::with_capacity(num_points);
    let dt = tof / (num_points as f64 - 1.0);

    let mut pos = launch_pos;
    let mut vel = departure_vel;

    // Simple two-body propagation (Sun-centered)
    for i in 0..num_points {
        points.push(pos);

        if i < num_points - 1 {
            // Simple Kepler propagation (Sun gravity only)
            let r = pos.length();
            let acc = -GM_SUN * pos / (r * r * r);

            // Velocity Verlet
            let half_vel = vel + acc * dt * 0.5;
            pos = pos + half_vel * dt;
            let r_new = pos.length();
            let acc_new = -GM_SUN * pos / (r_new * r_new * r_new);
            vel = half_vel + acc_new * dt * 0.5;
        }
    }

    points
}

/// Validate a Lambert solution by propagating and checking arrival accuracy.
///
/// Returns the arrival error distance if valid (< tolerance), None if invalid.
fn validate_lambert_solution(
    departure_pos: DVec2,
    departure_vel: DVec2,
    expected_arrival: DVec2,
    flight_time: f64,
    tolerance: f64,
) -> Option<f64> {
    let dt = 3600.0; // 1 hour timestep
    let mut pos = departure_pos;
    let mut vel = departure_vel;
    let mut t = 0.0;

    while t < flight_time {
        let step = (flight_time - t).min(dt);
        let r = pos.length();

        if r < 1e6 {
            return None; // Singularity
        }

        let acc = -GM_SUN * pos / (r * r * r);
        let half_vel = vel + acc * step * 0.5;
        pos = pos + half_vel * step;
        let r_new = pos.length();
        let acc_new = -GM_SUN * pos / (r_new * r_new * r_new);
        vel = half_vel + acc_new * step * 0.5;
        t += step;
    }

    let error = (pos - expected_arrival).length();
    if error <= tolerance {
        Some(error)
    } else {
        None
    }
}

/// Handle interceptor launch events.
///
/// Uses Lambert solver to compute realistic transfer orbit from Earth to
/// the predicted asteroid position at arrival time. If the asteroid would
/// collide with a celestial body before the arrival time, the flight time
/// is capped to arrive just before the collision.
#[allow(clippy::too_many_arguments)]
fn handle_launch_event(
    mut commands: Commands,
    mut events: MessageReader<LaunchInterceptorEvent>,
    mut registry: ResMut<InterceptorRegistry>,
    sim_time: Res<SimulationTime>,
    ephemeris: Res<Ephemeris>,
    asteroids: Query<&BodyState, With<Asteroid>>,
) {
    for event in events.read() {
        // Verify target exists
        let Ok(asteroid_state) = asteroids.get(event.target) else {
            warn!("Cannot launch interceptor: target asteroid not found");
            continue;
        };

        // Get Earth position and velocity for launch point
        let earth_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, sim_time.current)
            .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

        // Compute Earth's orbital velocity via numerical differentiation
        let dt = 60.0;
        let earth_pos_before = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, sim_time.current - dt)
            .unwrap_or(earth_pos);
        let earth_pos_after = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, sim_time.current + dt)
            .unwrap_or(earth_pos);
        let earth_vel = (earth_pos_after - earth_pos_before) / (2.0 * dt);

        // Determine deflection direction
        let direction = event.direction.unwrap_or_else(|| {
            // Default: retrograde (opposite to asteroid velocity)
            let vel_dir = -asteroid_state.vel.normalize_or_zero();
            if vel_dir == DVec2::ZERO {
                asteroid_state.pos.normalize_or_zero()
            } else {
                vel_dir
            }
        });

        // Requested flight time (default 90 days, must be positive)
        let requested_flight_time = event
            .flight_time
            .filter(|&t| t > 0.0 && t.is_finite())
            .unwrap_or(90.0 * 86400.0);

        // Predict asteroid trajectory with collision detection
        // Use 2x requested flight time to detect collisions that would occur before arrival
        let prediction = predict_asteroid_at_time_with_collision(
            asteroid_state,
            sim_time.current,
            sim_time.current + requested_flight_time * 2.0,
            &ephemeris,
        );

        // Determine actual flight time, capping at collision if needed
        let (flight_time, arrival_position, collision_warning) =
            if let Some((collision_body, collision_time)) = prediction.collision {
                // Collision will occur! Cap flight time to arrive before collision
                let time_to_collision = collision_time - sim_time.current;

                if time_to_collision <= 0.0 {
                    // Collision already happened or imminent - can't intercept
                    warn!(
                        "Cannot launch interceptor: asteroid collides with {:?} immediately",
                        collision_body
                    );
                    continue;
                }

                if time_to_collision < requested_flight_time {
                    // Collision before requested arrival - need to arrive earlier
                    // Aim for 90% of time-to-collision to have some margin
                    let capped_flight_time = time_to_collision * 0.9;

                    // Re-predict position at capped time
                    let (pos, _vel) = predict_asteroid_at_time(
                        asteroid_state,
                        sim_time.current,
                        sim_time.current + capped_flight_time,
                        &ephemeris,
                    );

                    warn!(
                        "Asteroid will collide with {:?} in {:.1} days - \
                         capping flight time from {:.1} to {:.1} days",
                        collision_body,
                        time_to_collision / 86400.0,
                        requested_flight_time / 86400.0,
                        capped_flight_time / 86400.0
                    );

                    (capped_flight_time, pos, Some(collision_body))
                } else {
                    // Collision after requested arrival - no capping needed
                    // But still use collision-aware prediction result
                    let (pos, _vel) = predict_asteroid_at_time(
                        asteroid_state,
                        sim_time.current,
                        sim_time.current + requested_flight_time,
                        &ephemeris,
                    );
                    (requested_flight_time, pos, None)
                }
            } else {
                // No collision detected - use requested flight time
                let (pos, _vel) = predict_asteroid_at_time(
                    asteroid_state,
                    sim_time.current,
                    sim_time.current + requested_flight_time,
                    &ephemeris,
                );
                (requested_flight_time, pos, None)
            };

        let arrival_time = sim_time.current + flight_time;

        // Try to solve Lambert's problem for transfer orbit
        // Uses auto-selection to try both prograde and retrograde, picking lower delta-v
        let (transfer_arc, departure_velocity) =
            match solve_lambert_auto(earth_pos, arrival_position, flight_time, GM_SUN) {
                Some(solution) => {
                    // Validate: propagated trajectory should reach target within 0.01 AU
                    let tolerance = 0.01 * AU_TO_METERS;

                    match validate_lambert_solution(
                        earth_pos,
                        solution.v1,
                        arrival_position,
                        flight_time,
                        tolerance,
                    ) {
                        Some(error) => {
                            // Generate arc points for visualization
                            let arc = generate_transfer_arc(
                                earth_pos,
                                solution.v1,
                                flight_time,
                                TRANSFER_ARC_POINTS,
                            );

                            // Spacecraft delta-v is departure velocity minus Earth velocity
                            let spacecraft_dv = (solution.v1 - earth_vel).length();

                            info!(
                                "Lambert solution: Δv = {:.2} km/s, arrival error = {:.4} AU",
                                spacecraft_dv / 1000.0,
                                error / AU_TO_METERS
                            );

                            (arc, solution.v1)
                        }
                        None => {
                            warn!(
                                "Lambert solution failed validation (arrival > 0.01 AU from target)"
                            );
                            (Vec::new(), DVec2::ZERO)
                        }
                    }
                }
                None => {
                    // Fallback: no curved arc, just linear interpolation
                    warn!("Lambert solver did not converge, using linear trajectory");
                    (Vec::new(), DVec2::ZERO)
                }
            };

        // Create interceptor entity with unique ID
        registry.total_launched += 1;
        let interceptor_id = registry.total_launched;

        let interceptor = Interceptor {
            id: interceptor_id,
            target: event.target,
            payload: event.payload.clone(),
            launch_time: sim_time.current,
            arrival_time,
            launch_position: earth_pos,
            arrival_position,
            deflection_direction: direction.normalize_or_zero(),
            state: InterceptorState::InFlight,
            transfer_arc,
            departure_velocity,
        };

        commands.spawn(interceptor.clone());

        let collision_note = collision_warning
            .map(|body| format!(" (capped due to {:?} collision)", body))
            .unwrap_or_default();

        info!(
            "Interceptor #{} launched: {} targeting asteroid, ETA {:.1} days{}",
            registry.total_launched,
            interceptor.payload.description(),
            flight_time / 86400.0,
            collision_note
        );
    }
}

/// Update interceptors and apply delta-v on arrival.
#[allow(clippy::too_many_arguments)]
fn update_interceptors(
    mut commands: Commands,
    mut interceptors: Query<(Entity, &mut Interceptor)>,
    mut asteroids: Query<(&mut BodyState, &AsteroidName), With<Asteroid>>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    mut split_events: MessageWriter<SplitAsteroidEvent>,
    mut impact_effects: MessageWriter<crate::render::SpawnImpactEffectEvent>,
    sim_time: Res<SimulationTime>,
) {
    use crate::render::ImpactEffectType;

    for (entity, mut interceptor) in interceptors.iter_mut() {
        // Skip if not in flight
        if interceptor.state != InterceptorState::InFlight {
            continue;
        }

        // Check if arrived
        if sim_time.current >= interceptor.arrival_time {
            // Get asteroid state
            let Ok((mut asteroid_state, asteroid_name)) = asteroids.get_mut(interceptor.target)
            else {
                // Target destroyed or missing
                interceptor.state = InterceptorState::Cancelled;
                commands.entity(entity).despawn();
                continue;
            };

            // Check if this is a splitting payload
            if let DeflectionPayload::NuclearSplit {
                yield_kt,
                split_ratio,
            } = &interceptor.payload
            {
                // Send splitting event instead of applying delta-v
                split_events.write(SplitAsteroidEvent {
                    target: interceptor.target,
                    position: asteroid_state.pos,
                    velocity: asteroid_state.vel,
                    mass: asteroid_state.mass,
                    original_name: asteroid_name.0.clone(),
                    yield_kt: *yield_kt,
                    split_ratio: *split_ratio,
                    deflection_direction: interceptor.deflection_direction,
                });

                // Spawn nuclear split visual effect
                impact_effects.write(crate::render::SpawnImpactEffectEvent {
                    position: asteroid_state.pos,
                    effect_type: ImpactEffectType::NuclearSplit {
                        yield_kt: *yield_kt,
                    },
                });

                info!(
                    "Nuclear split initiated! {} kt detonation on {}, creating two fragments",
                    yield_kt, asteroid_name.0
                );
            } else {
                // Standard deflection - apply delta-v
                let relative_velocity = asteroid_state.vel.length();

                let delta_v = interceptor.payload.calculate_delta_v(
                    asteroid_state.mass,
                    relative_velocity,
                    interceptor.deflection_direction,
                );

                asteroid_state.vel += delta_v;

                // Reset integrator state for new velocity
                integrator_states.remove(interceptor.target);

                // Trigger trajectory recalculation
                mark_prediction_dirty(&mut prediction_state);

                // Spawn appropriate impact effect based on payload type
                let effect_type = match &interceptor.payload {
                    DeflectionPayload::Kinetic { .. } => {
                        ImpactEffectType::KineticFlash { intensity: 1.0 }
                    }
                    DeflectionPayload::Nuclear { yield_kt } => ImpactEffectType::NuclearExplosion {
                        yield_kt: *yield_kt,
                    },
                    DeflectionPayload::NuclearSplit { .. } => unreachable!(),
                };
                impact_effects.write(crate::render::SpawnImpactEffectEvent {
                    position: asteroid_state.pos,
                    effect_type,
                });

                info!(
                    "Interceptor impact! Applied Δv = {:.4} mm/s in direction ({:.2}, {:.2})",
                    delta_v.length() * 1000.0,
                    interceptor.deflection_direction.x,
                    interceptor.deflection_direction.y
                );
            }

            // Mark as arrived and despawn
            interceptor.state = InterceptorState::Arrived;
            commands.entity(entity).despawn();
        }
    }
}

/// Draw interceptor trajectories using gizmos.
///
/// Uses the pre-computed transfer arc if available, falls back to linear interpolation.
/// Color varies by interceptor ID for visual distinction when multiple are in flight.
fn draw_interceptor_trajectories(
    interceptors: Query<&Interceptor>,
    asteroids: Query<&BodyState, With<Asteroid>>,
    sim_time: Res<SimulationTime>,
    mut gizmos: Gizmos,
) {
    for interceptor in interceptors.iter() {
        if interceptor.state != InterceptorState::InFlight {
            continue;
        }

        // Get current asteroid position for final segment (asteroid may have moved)
        let Ok(asteroid_state) = asteroids.get(interceptor.target) else {
            continue;
        };

        // Calculate progress along trajectory
        let total_time = interceptor.arrival_time - interceptor.launch_time;
        let elapsed = sim_time.current - interceptor.launch_time;
        let progress = (elapsed / total_time).clamp(0.0, 1.0);

        // Base color based on payload type, with hue shift for ID differentiation
        let base_color = match &interceptor.payload {
            DeflectionPayload::Kinetic { .. } => Color::srgb(1.0, 1.0, 1.0), // White
            DeflectionPayload::Nuclear { .. } => Color::srgb(1.0, 0.6, 0.2), // Orange
            DeflectionPayload::NuclearSplit { .. } => Color::srgb(1.0, 0.2, 0.4), // Red/Pink
        };

        // Apply hue shift based on interceptor ID for multiple launch distinction
        let color = shift_hue_by_id(base_color, interceptor.id);

        let z = 2.5; // Above planets, below asteroid

        // Use transfer arc if available, otherwise fall back to linear
        if !interceptor.transfer_arc.is_empty() {
            let num_points = interceptor.transfer_arc.len();
            let current_index = (progress * (num_points - 1) as f64) as usize;
            let current_index = current_index.min(num_points - 1);

            // Draw traveled portion (solid, full color)
            for i in 0..current_index {
                let p1 = (interceptor.transfer_arc[i] * RENDER_SCALE).as_vec2();
                let p2 = (interceptor.transfer_arc[i + 1] * RENDER_SCALE).as_vec2();
                gizmos.line(Vec3::new(p1.x, p1.y, z), Vec3::new(p2.x, p2.y, z), color);
            }

            // Interpolate current position within the arc segment
            let segment_progress = (progress * (num_points - 1) as f64).fract();
            let current_pos = if current_index < num_points - 1 {
                interceptor.transfer_arc[current_index].lerp(
                    interceptor.transfer_arc[current_index + 1],
                    segment_progress,
                )
            } else {
                interceptor.transfer_arc[current_index]
            };

            // Draw remaining portion (semi-transparent)
            let remaining_color = color.with_alpha(0.4);
            for i in current_index..(num_points - 1) {
                let p1 = if i == current_index {
                    (current_pos * RENDER_SCALE).as_vec2()
                } else {
                    (interceptor.transfer_arc[i] * RENDER_SCALE).as_vec2()
                };
                let p2 = (interceptor.transfer_arc[i + 1] * RENDER_SCALE).as_vec2();
                gizmos.line(
                    Vec3::new(p1.x, p1.y, z),
                    Vec3::new(p2.x, p2.y, z),
                    remaining_color,
                );
            }

            // Draw final segment to current asteroid position (it may have moved)
            let last_arc_point =
                (interceptor.transfer_arc[num_points - 1] * RENDER_SCALE).as_vec2();
            let target_render = (asteroid_state.pos * RENDER_SCALE).as_vec2();
            gizmos.line(
                Vec3::new(last_arc_point.x, last_arc_point.y, z),
                Vec3::new(target_render.x, target_render.y, z),
                remaining_color,
            );

            // Draw interceptor icon at current position
            let current_render = (current_pos * RENDER_SCALE).as_vec2();
            draw_interceptor_icon(&mut gizmos, current_render, z, color);
        } else {
            // Fallback: linear interpolation
            let current_pos = interceptor
                .launch_position
                .lerp(asteroid_state.pos, progress);

            let launch_render = (interceptor.launch_position * RENDER_SCALE).as_vec2();
            let current_render = (current_pos * RENDER_SCALE).as_vec2();
            let target_render = (asteroid_state.pos * RENDER_SCALE).as_vec2();

            // Traveled portion (solid)
            gizmos.line(
                Vec3::new(launch_render.x, launch_render.y, z),
                Vec3::new(current_render.x, current_render.y, z),
                color,
            );

            // Remaining portion (semi-transparent)
            gizmos.line(
                Vec3::new(current_render.x, current_render.y, z),
                Vec3::new(target_render.x, target_render.y, z),
                color.with_alpha(0.4),
            );

            draw_interceptor_icon(&mut gizmos, current_render, z, color);
        }
    }
}

/// Shift hue of a color based on interceptor ID for visual distinction.
///
/// Uses HSL color space to shift hue by 30° per ID (mod 12),
/// maintaining the original saturation and lightness.
fn shift_hue_by_id(color: Color, id: u32) -> Color {
    // For ID 1 (first interceptor), no shift - use base color
    if id <= 1 {
        return color;
    }

    // Convert to linear RGBA
    let linear = color.to_linear();
    let r = linear.red;
    let g = linear.green;
    let b = linear.blue;

    // Convert RGB to HSL
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;

    if (max - min).abs() < 0.001 {
        // Achromatic (white/gray), can't shift hue meaningfully
        // Apply a slight color tint instead
        let hue_offset = ((id - 1) % 12) as f32 * 30.0 / 360.0;
        let tint = Color::hsl(hue_offset * 360.0, 0.5, l);
        return tint;
    }

    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };

    let h = if (max - r).abs() < 0.001 {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if (max - g).abs() < 0.001 {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };

    // Shift hue by 30° per ID (cycles through 12 colors)
    let hue_shift = ((id - 1) % 12) as f32 * (30.0 / 360.0);
    let new_h = (h + hue_shift) % 1.0;

    Color::hsl(new_h * 360.0, s, l)
}

/// Draw a diamond-shaped interceptor icon at the given position.
fn draw_interceptor_icon(gizmos: &mut Gizmos, pos: Vec2, z: f32, color: Color) {
    let size = 0.01 * AU_TO_METERS as f32 * RENDER_SCALE as f32;
    gizmos.line(
        Vec3::new(pos.x - size, pos.y, z),
        Vec3::new(pos.x, pos.y + size, z),
        color,
    );
    gizmos.line(
        Vec3::new(pos.x, pos.y + size, z),
        Vec3::new(pos.x + size, pos.y, z),
        color,
    );
    gizmos.line(
        Vec3::new(pos.x + size, pos.y, z),
        Vec3::new(pos.x, pos.y - size, z),
        color,
    );
    gizmos.line(
        Vec3::new(pos.x, pos.y - size, z),
        Vec3::new(pos.x - size, pos.y, z),
        color,
    );
}

/// Estimate optimal deflection direction for a scenario.
///
/// For maximum miss distance with given lead time:
/// - Retrograde (opposite velocity) is usually best for direct impact
/// - Prograde can work for longer lead times
/// - Perpendicular (orbit plane) for maximum lateral deflection
pub fn optimal_deflection_direction(asteroid_vel: DVec2, _lead_time: f64) -> DVec2 {
    // Simplified: always use retrograde for now
    // A more sophisticated version would consider the geometry
    -asteroid_vel.normalize_or_zero()
}

/// Handle asteroid splitting events.
///
/// This system:
/// 1. Despawns the original asteroid
/// 2. Creates two fragment asteroids with diverging trajectories
/// 3. Conserves momentum while adding separation velocity from the explosion
#[allow(clippy::too_many_arguments)]
fn handle_asteroid_splitting(
    mut commands: Commands,
    mut events: MessageReader<SplitAsteroidEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut counter: ResMut<AsteroidCounter>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    trajectories: Query<&TrajectoryPath, With<Asteroid>>,
) {
    for event in events.read() {
        // Validate split ratio to prevent negative or impossible masses
        let split_ratio = event.split_ratio.clamp(0.01, 0.99);

        // Calculate fragment masses
        let mass1 = event.mass * split_ratio;
        let mass2 = event.mass * (1.0 - split_ratio);

        // Calculate separation velocity from nuclear explosion
        let separation_speed =
            DeflectionPayload::calculate_separation_velocity(event.yield_kt, event.mass);

        // Separation direction is perpendicular to deflection direction
        // (fragments fly apart sideways relative to the thrust direction)
        let separation_dir =
            DVec2::new(-event.deflection_direction.y, event.deflection_direction.x)
                .normalize_or_zero();

        // Each fragment gets velocity proportional to the other's mass (momentum conservation)
        // v1 * m1 = v2 * m2 for the separation component
        let v1_separation = separation_dir * separation_speed * (mass2 / event.mass);
        let v2_separation = -separation_dir * separation_speed * (mass1 / event.mass);

        // Fragment velocities = original velocity + separation velocity
        let vel1 = event.velocity + v1_separation;
        let vel2 = event.velocity + v2_separation;

        // Small position offset so fragments don't start at exact same location
        let offset_dist = 1000.0; // 1 km offset
        let pos1 = event.position + separation_dir * offset_dist;
        let pos2 = event.position - separation_dir * offset_dist;

        // Remove integrator state for original asteroid
        integrator_states.remove(event.target);

        // Preserve trajectory color if available
        let _original_trajectory = trajectories.get(event.target).ok();

        // Despawn original asteroid
        commands.entity(event.target).despawn();

        // Spawn fragment 1 with unique color
        counter.0 += 1;
        let name1 = format!("{} Fragment A", event.original_name);
        let color1 = crate::asteroid::asteroid_color(counter.0);
        let entity1 = spawn_asteroid(
            &mut commands,
            &mut meshes,
            &mut materials,
            name1.clone(),
            pos1,
            vel1,
            mass1,
            color1,
        );

        // Spawn fragment 2 with unique color
        counter.0 += 1;
        let name2 = format!("{} Fragment B", event.original_name);
        let color2 = crate::asteroid::asteroid_color(counter.0);
        let entity2 = spawn_asteroid(
            &mut commands,
            &mut meshes,
            &mut materials,
            name2.clone(),
            pos2,
            vel2,
            mass2,
            color2,
        );

        // Mark predictions dirty
        mark_prediction_dirty(&mut prediction_state);

        info!(
            "Asteroid split complete! {} ({:.2e} kg, v={:.1} m/s) and {} ({:.2e} kg, v={:.1} m/s)",
            name1,
            mass1,
            vel1.length(),
            name2,
            mass2,
            vel2.length()
        );
        info!(
            "Separation velocity: {:.2} m/s (entities {:?} and {:?})",
            separation_speed, entity1, entity2
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interceptor_state_default() {
        let interceptor = Interceptor {
            id: 1,
            target: Entity::PLACEHOLDER,
            payload: DeflectionPayload::default(),
            launch_time: 0.0,
            arrival_time: 86400.0,
            launch_position: DVec2::ZERO,
            arrival_position: DVec2::new(AU_TO_METERS, 0.0),
            deflection_direction: DVec2::X,
            state: InterceptorState::InFlight,
            transfer_arc: Vec::new(),
            departure_velocity: DVec2::ZERO,
        };
        assert_eq!(interceptor.state, InterceptorState::InFlight);
    }

    #[test]
    fn test_optimal_direction_retrograde() {
        let vel = DVec2::new(10000.0, 5000.0);
        let direction = optimal_deflection_direction(vel, 180.0 * 86400.0);

        // Should be opposite to velocity
        let dot = direction.dot(vel.normalize());
        assert!(
            (dot + 1.0).abs() < 1e-10,
            "Should be retrograde, dot = {dot}"
        );
    }

    /// Test that asteroid prediction detects collisions.
    ///
    /// This test verifies that predict_asteroid_at_time_with_collision()
    /// correctly detects when an asteroid enters Earth's collision radius.
    #[test]
    fn test_prediction_detects_collision() {
        use crate::ephemeris::COLLISION_MULTIPLIER;

        let ephemeris = Ephemeris::default();

        // Get Earth's position and data at J2000
        let earth_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, 0.0)
            .unwrap();
        let earth_data = ephemeris
            .get_body_data_by_id(CelestialBodyId::Earth)
            .unwrap();
        let earth_collision_radius = earth_data.radius * COLLISION_MULTIPLIER;

        println!("Earth position: ({:.3e}, {:.3e})", earth_pos.x, earth_pos.y);
        println!(
            "Earth collision radius: {:.0} km ({:.1}x actual radius)",
            earth_collision_radius / 1000.0,
            COLLISION_MULTIPLIER
        );

        // Put asteroid just outside collision radius, moving fast toward Earth
        // Start at 400,000 km from Earth (collision radius is ~318,000 km)
        // Use Earth's velocity direction to set up a lead collision

        // Get Earth's approximate orbital velocity (tangent to orbit)
        // Earth at 1 AU moves at ~30 km/s
        let earth_distance = earth_pos.length();
        let earth_speed = (crate::types::GM_SUN / earth_distance).sqrt();
        let earth_vel_dir = DVec2::new(-earth_pos.y, earth_pos.x).normalize();
        let earth_vel = earth_vel_dir * earth_speed;

        println!("Earth orbital speed: {:.1} km/s", earth_speed / 1000.0);

        // Position asteroid 400,000 km radially outward from Earth
        let radial_dir = earth_pos.normalize();
        let asteroid_pos = earth_pos + radial_dir * 400_000_000.0; // 400,000 km in meters

        // Give asteroid velocity toward Earth PLUS Earth's orbital velocity
        // so it actually intercepts Earth rather than missing behind it
        let to_earth = (earth_pos - asteroid_pos).normalize();
        let closing_speed = 50_000.0; // 50 km/s toward Earth
        let asteroid_vel = earth_vel + to_earth * closing_speed;

        let initial_state = BodyState {
            pos: asteroid_pos,
            vel: asteroid_vel,
            mass: 1e10,
        };

        // Distance to collision: 400,000 km - 318,000 km = 82,000 km
        // At 50 km/s closing speed: ~1640 seconds = ~27 minutes
        let distance_to_collision = 400_000_000.0 - earth_collision_radius;
        let expected_collision_time = distance_to_collision / closing_speed;

        println!("\nDistance to Earth center: {:.0} km", 400_000.0);
        println!(
            "Distance to collision radius: {:.0} km",
            distance_to_collision / 1000.0
        );
        println!("Closing speed: {:.1} km/s", closing_speed / 1000.0);
        println!(
            "Expected collision time: {:.1} minutes ({:.0} seconds)",
            expected_collision_time / 60.0,
            expected_collision_time
        );

        // Try to predict position at 2x the collision time
        let target_time = expected_collision_time * 2.0;
        let result =
            predict_asteroid_at_time_with_collision(&initial_state, 0.0, target_time, &ephemeris);

        println!("\nPrediction result:");
        println!("  Collision detected: {}", result.collision.is_some());
        if let Some((collision_body, collision_time)) = &result.collision {
            println!("  Collision body: {:?}", collision_body);
            println!(
                "  Collision time: {:.1} minutes ({:.0} seconds)",
                collision_time / 60.0,
                collision_time
            );
        }
        println!(
            "  Final position: ({:.3e}, {:.3e})",
            result.pos.x, result.pos.y
        );

        // Get Earth's position at collision/final time to compare
        let check_time = result.collision.map(|(_, t)| t).unwrap_or(target_time);
        let earth_pos_at_check = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, check_time)
            .unwrap();
        println!(
            "  Earth position at that time: ({:.3e}, {:.3e})",
            earth_pos_at_check.x, earth_pos_at_check.y
        );
        println!(
            "  Distance from Earth at result: {:.0} km",
            (result.pos - earth_pos_at_check).length() / 1000.0
        );

        // THE CORE ASSERTION: Collision should be detected!
        assert!(
            result.collision.is_some(),
            "\n\nBUG: Collision not detected!\n\
             \n\
             An asteroid starting 400,000 km from Earth and moving toward it at 50 km/s\n\
             (plus Earth's orbital velocity) should collide in ~27 minutes.\n\
             \n\
             This causes interceptors to aim at positions past the collision point.\n\
             \n\
             Fix: predict_asteroid_at_time_with_collision() needs to check for\n\
             collision at each timestep using ephemeris.check_collision()."
        );

        // Also verify the collision was with Earth
        assert_eq!(
            result.collision.map(|(body, _)| body),
            Some(CelestialBodyId::Earth),
            "Collision should be with Earth"
        );
    }

    #[test]
    fn test_validation_accepts_good_solution() {
        use crate::lambert::solve_lambert_auto;

        // Earth position at 1 AU on x-axis
        let r1 = DVec2::new(AU_TO_METERS, 0.0);
        // Target at 1 AU on y-axis (90-degree transfer)
        let r2 = DVec2::new(0.0, AU_TO_METERS);
        // ~91 days for a quarter-orbit Hohmann-like transfer
        let tof = 91.0 * 86400.0;

        let solution = solve_lambert_auto(r1, r2, tof, GM_SUN).expect("Lambert should converge");
        let result = validate_lambert_solution(r1, solution.v1, r2, tof, 0.01 * AU_TO_METERS);

        assert!(
            result.is_some(),
            "Valid Lambert solution should pass validation"
        );
        let error = result.unwrap();
        assert!(
            error < 0.001 * AU_TO_METERS,
            "Arrival error should be < 0.001 AU, got {:.6} AU",
            error / AU_TO_METERS
        );
    }

    #[test]
    fn test_validation_rejects_bad_velocity() {
        let r1 = DVec2::new(AU_TO_METERS, 0.0);
        let r2 = DVec2::new(0.0, AU_TO_METERS);
        let tof = 91.0 * 86400.0;

        // Wrong velocity - just some arbitrary direction
        let wrong_vel = DVec2::new(30000.0, 0.0);
        let result = validate_lambert_solution(r1, wrong_vel, r2, tof, 0.01 * AU_TO_METERS);

        assert!(result.is_none(), "Bad velocity should fail validation");
    }
}
