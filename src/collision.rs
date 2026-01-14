//! Collision detection for asteroid-celestial body impacts.
//!
//! Monitors asteroid positions and detects when they collide with
//! any celestial body. On collision:
//! - The simulation is paused
//! - The asteroid is destroyed
//! - A notification is shown
//!
//! The user can resume simulation after reviewing the impact.

use bevy::prelude::*;
use bevy::math::DVec2;

use crate::asteroid::{Asteroid, AsteroidName};
use crate::ephemeris::{data::CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::render::SelectedBody;
use crate::types::{BodyState, SelectableBody, SimulationTime, SECONDS_PER_DAY};

/// Event fired when an asteroid collides with a celestial body.
///
/// This event is sent when collision is detected, allowing UI systems
/// to display impact information. The asteroid is destroyed after the event.
#[derive(Event, Clone, Debug)]
pub struct CollisionEvent {
    /// Name of the asteroid that collided (entity is destroyed).
    pub asteroid_name: String,
    /// The celestial body that was hit.
    pub body_hit: CelestialBodyId,
    /// Position of impact in meters from barycenter.
    pub impact_position: DVec2,
    /// Velocity at impact in m/s.
    pub impact_velocity: DVec2,
    /// Simulation time of impact (seconds since J2000).
    pub time: f64,
}

impl CollisionEvent {
    /// Get the impact velocity magnitude in km/s.
    pub fn impact_speed_km_s(&self) -> f64 {
        self.impact_velocity.length() / 1000.0
    }

    /// Get the simulation time of impact in days since J2000.
    pub fn time_days(&self) -> f64 {
        self.time / SECONDS_PER_DAY
    }
}

/// Resource tracking collision state for UI display.
///
/// Stores the most recent collision event so the UI can display
/// impact information even after the event has been consumed.
#[derive(Resource, Default)]
pub struct CollisionState {
    /// Most recent collision, if any.
    pub last_collision: Option<CollisionEvent>,
}

impl CollisionState {
    /// Clear the collision state (e.g., when resetting scenario).
    pub fn clear(&mut self) {
        self.last_collision = None;
    }

    /// Check if there's an active collision.
    pub fn has_collision(&self) -> bool {
        self.last_collision.is_some()
    }
}

/// Check for collisions between asteroids and celestial bodies.
///
/// This system runs after physics integration. When a collision is detected:
/// 1. The simulation is paused
/// 2. The asteroid is destroyed
/// 3. A CollisionEvent is sent for UI notification
/// 4. The CollisionState is updated
///
/// Each asteroid collision is handled independently. After the user resumes
/// the simulation, it continues with the remaining asteroids.
pub fn check_collisions(
    mut commands: Commands,
    asteroids: Query<(Entity, &AsteroidName, &BodyState), With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
    mut sim_time: ResMut<SimulationTime>,
    mut collision_events: EventWriter<CollisionEvent>,
    mut collision_state: ResMut<CollisionState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut selected: ResMut<SelectedBody>,
) {
    // Skip if already paused (avoid repeated collision events while paused)
    if sim_time.paused {
        return;
    }

    // Collect collisions first to avoid borrow conflicts
    let mut collisions = Vec::new();

    for (entity, name, body_state) in asteroids.iter() {
        // Check collision using ephemeris
        if let Some(body_hit) = ephemeris.check_collision(body_state.pos, sim_time.current) {
            collisions.push((entity, name.0.clone(), body_state.clone(), body_hit));
        }
    }

    // Process all collisions
    for (entity, asteroid_name, body_state, body_hit) in collisions {
        // Create collision event
        let event = CollisionEvent {
            asteroid_name: asteroid_name.clone(),
            body_hit,
            impact_position: body_state.pos,
            impact_velocity: body_state.vel,
            time: sim_time.current,
        };

        info!(
            "IMPACT! {} hit {:?} at {:.2} km/s",
            asteroid_name,
            body_hit,
            event.impact_speed_km_s(),
        );

        // Destroy the asteroid
        commands.entity(entity).despawn();
        integrator_states.remove(entity);

        // Clear selection if this asteroid was selected
        if selected.body == Some(SelectableBody::Asteroid(entity)) {
            selected.body = None;
        }

        // Fire event and store state
        collision_events.send(event.clone());
        collision_state.last_collision = Some(event);

        // Pause simulation (will pause on first collision, additional collisions
        // in the same frame are still processed and destroyed)
        sim_time.paused = true;
    }
}

/// Plugin providing collision detection for asteroids.
pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CollisionEvent>()
            .insert_resource(CollisionState::default())
            // Run collision detection in FixedUpdate after physics
            // No explicit ordering needed - just runs after physics_step
            .add_systems(FixedUpdate, check_collisions);
    }
}
