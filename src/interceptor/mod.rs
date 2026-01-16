//! Interceptor system for deflecting asteroids.
//!
//! Provides mechanics for launching interceptors from Earth to deflect asteroids:
//! - Kinetic impactors (DART-style)
//! - Nuclear standoff detonations
//!
//! Interceptors travel on simplified trajectories and apply delta-v on arrival.

pub mod payload;

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::asteroid::Asteroid;
use crate::camera::RENDER_SCALE;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::prediction::{mark_prediction_dirty, PredictionState};
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

/// Plugin for interceptor management.
pub struct InterceptorPlugin;

impl Plugin for InterceptorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InterceptorRegistry>()
            .add_event::<LaunchInterceptorEvent>()
            .add_systems(
                Update,
                (
                    handle_launch_event,
                    update_interceptors,
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
    mut asteroids: Query<&mut BodyState, With<Asteroid>>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut prediction_state: ResMut<PredictionState>,
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
            let Ok(mut asteroid_state) = asteroids.get_mut(interceptor.target) else {
                // Target destroyed or missing
                interceptor.state = InterceptorState::Cancelled;
                commands.entity(entity).despawn();
                continue;
            };

            // Calculate relative velocity (simplified: use current asteroid velocity)
            // In reality, this would depend on the intercept geometry
            let relative_velocity = asteroid_state.vel.length();

            // Calculate and apply delta-v
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
