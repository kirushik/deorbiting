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

use crate::asteroid::{spawn_asteroid, Asteroid, AsteroidCounter, AsteroidName};
use crate::camera::RENDER_SCALE;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::prediction::{mark_prediction_dirty, PredictionState, TrajectoryPath};
use crate::types::{BodyState, SimulationTime, AU_TO_METERS};

pub use payload::DeflectionPayload;

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
    /// Deflection direction (unit vector).
    pub deflection_direction: DVec2,
    /// Current state.
    pub state: InterceptorState,
}

/// Resource for tracking active interceptors.
#[derive(Resource, Default)]
pub struct InterceptorRegistry {
    /// Count of launched interceptors.
    pub total_launched: u32,
}

/// Event to launch a new interceptor.
#[derive(Event)]
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
#[derive(Event)]
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
            .add_event::<LaunchInterceptorEvent>()
            .add_event::<SplitAsteroidEvent>()
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

/// Handle interceptor launch events.
#[allow(clippy::too_many_arguments)]
fn handle_launch_event(
    mut commands: Commands,
    mut events: EventReader<LaunchInterceptorEvent>,
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

        // Get Earth position for launch point
        let earth_pos = ephemeris.get_position_by_id(CelestialBodyId::Earth, sim_time.current)
            .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

        // Determine deflection direction
        let direction = event.direction.unwrap_or_else(|| {
            // Default: retrograde (opposite to asteroid velocity)
            -asteroid_state.vel.normalize_or_zero()
        });

        // Flight time (default 90 days)
        let flight_time = event.flight_time.unwrap_or(90.0 * 86400.0);

        // Create interceptor entity
        let interceptor = Interceptor {
            target: event.target,
            payload: event.payload.clone(),
            launch_time: sim_time.current,
            arrival_time: sim_time.current + flight_time,
            launch_position: earth_pos,
            deflection_direction: direction.normalize_or_zero(),
            state: InterceptorState::InFlight,
        };

        commands.spawn(interceptor.clone());
        registry.total_launched += 1;

        info!(
            "Interceptor #{} launched: {} targeting asteroid, ETA {:.1} days",
            registry.total_launched,
            interceptor.payload.description(),
            flight_time / 86400.0
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
    mut split_events: EventWriter<SplitAsteroidEvent>,
    sim_time: Res<SimulationTime>,
) {
    for (entity, mut interceptor) in interceptors.iter_mut() {
        // Skip if not in flight
        if interceptor.state != InterceptorState::InFlight {
            continue;
        }

        // Check if arrived
        if sim_time.current >= interceptor.arrival_time {
            // Get asteroid state
            let Ok((mut asteroid_state, asteroid_name)) = asteroids.get_mut(interceptor.target) else {
                // Target destroyed or missing
                interceptor.state = InterceptorState::Cancelled;
                commands.entity(entity).despawn();
                continue;
            };

            // Check if this is a splitting payload
            if let DeflectionPayload::NuclearSplit { yield_kt, split_ratio } = &interceptor.payload {
                // Send splitting event instead of applying delta-v
                split_events.send(SplitAsteroidEvent {
                    target: interceptor.target,
                    position: asteroid_state.pos,
                    velocity: asteroid_state.vel,
                    mass: asteroid_state.mass,
                    original_name: asteroid_name.0.clone(),
                    yield_kt: *yield_kt,
                    split_ratio: *split_ratio,
                    deflection_direction: interceptor.deflection_direction,
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

        // Get current asteroid position as target
        let Ok(asteroid_state) = asteroids.get(interceptor.target) else {
            continue;
        };

        // Calculate interpolated interceptor position
        let total_time = interceptor.arrival_time - interceptor.launch_time;
        let elapsed = sim_time.current - interceptor.launch_time;
        let progress = (elapsed / total_time).clamp(0.0, 1.0);

        // Simple linear interpolation (not physically accurate, but visually useful)
        let current_pos = interceptor.launch_position.lerp(asteroid_state.pos, progress);

        // Convert to render coordinates
        let launch_render = (interceptor.launch_position * RENDER_SCALE).as_vec2();
        let current_render = (current_pos * RENDER_SCALE).as_vec2();
        let target_render = (asteroid_state.pos * RENDER_SCALE).as_vec2();

        // Color based on payload type
        let color = match &interceptor.payload {
            DeflectionPayload::Kinetic { .. } => Color::srgb(1.0, 1.0, 1.0), // White
            DeflectionPayload::Nuclear { .. } => Color::srgb(1.0, 0.6, 0.2), // Orange
            DeflectionPayload::NuclearSplit { .. } => Color::srgb(1.0, 0.2, 0.4), // Red/Pink (danger!)
        };

        // Draw trajectory as dotted line
        let z = 2.5; // Above planets, below asteroid

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

        // Draw interceptor icon (small diamond at current position)
        let size = 0.01 * AU_TO_METERS as f32 * RENDER_SCALE as f32; // Scale with AU
        gizmos.line(
            Vec3::new(current_render.x - size, current_render.y, z),
            Vec3::new(current_render.x, current_render.y + size, z),
            color,
        );
        gizmos.line(
            Vec3::new(current_render.x, current_render.y + size, z),
            Vec3::new(current_render.x + size, current_render.y, z),
            color,
        );
        gizmos.line(
            Vec3::new(current_render.x + size, current_render.y, z),
            Vec3::new(current_render.x, current_render.y - size, z),
            color,
        );
        gizmos.line(
            Vec3::new(current_render.x, current_render.y - size, z),
            Vec3::new(current_render.x - size, current_render.y, z),
            color,
        );
    }
}

/// Estimate optimal deflection direction for a scenario.
///
/// For maximum miss distance with given lead time:
/// - Retrograde (opposite velocity) is usually best for direct impact
/// - Prograde can work for longer lead times
/// - Perpendicular (orbit plane) for maximum lateral deflection
pub fn optimal_deflection_direction(
    asteroid_vel: DVec2,
    _lead_time: f64,
) -> DVec2 {
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
    mut events: EventReader<SplitAsteroidEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut counter: ResMut<AsteroidCounter>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
    trajectories: Query<&TrajectoryPath, With<Asteroid>>,
) {
    for event in events.read() {
        // Calculate fragment masses
        let mass1 = event.mass * event.split_ratio;
        let mass2 = event.mass * (1.0 - event.split_ratio);

        // Calculate separation velocity from nuclear explosion
        let separation_speed = DeflectionPayload::calculate_separation_velocity(event.yield_kt, event.mass);

        // Separation direction is perpendicular to deflection direction
        // (fragments fly apart sideways relative to the thrust direction)
        let separation_dir = DVec2::new(-event.deflection_direction.y, event.deflection_direction.x).normalize_or_zero();

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
        commands.entity(event.target).despawn_recursive();

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
            name1, mass1, vel1.length(),
            name2, mass2, vel2.length()
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
            target: Entity::PLACEHOLDER,
            payload: DeflectionPayload::default(),
            launch_time: 0.0,
            arrival_time: 86400.0,
            launch_position: DVec2::ZERO,
            deflection_direction: DVec2::X,
            state: InterceptorState::InFlight,
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
}
