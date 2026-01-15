//! Collision detection for asteroid-celestial body impacts.
//!
//! Monitors asteroid positions and detects when they collide with
//! any celestial body. On collision:
//! - The asteroid is immediately marked as colliding (excluded from physics)
//! - A notification is queued for display
//! - The simulation is paused
//! - The asteroid is destroyed
//!
//! The user can resume simulation after reviewing the impact.

use std::collections::{HashSet, VecDeque};

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

/// Resource tracking collision state for UI display and physics exclusion.
///
/// Uses a queue-based architecture to handle multiple collisions properly:
/// - `pending_notifications`: Queue of collision events waiting to be displayed
/// - `colliding_entities`: Set of entities marked as colliding (excluded from physics)
#[derive(Resource, Default)]
pub struct CollisionState {
    /// Queue of collision notifications to display (FIFO order).
    pub pending_notifications: VecDeque<CollisionEvent>,
    /// Entities currently colliding (excluded from physics until despawned).
    /// This provides immediate exclusion before the deferred despawn executes.
    pub colliding_entities: HashSet<Entity>,
}

impl CollisionState {
    /// Clear all collision state (e.g., when resetting scenario).
    pub fn clear(&mut self) {
        self.pending_notifications.clear();
        self.colliding_entities.clear();
    }

    /// Record a collision: mark entity as colliding and queue notification.
    pub fn push_collision(&mut self, entity: Entity, event: CollisionEvent) {
        self.colliding_entities.insert(entity);
        self.pending_notifications.push_back(event);
    }

    /// Pop the next notification from the queue (for UI display).
    pub fn pop_notification(&mut self) -> Option<CollisionEvent> {
        self.pending_notifications.pop_front()
    }

    /// Check if there are pending notifications.
    pub fn has_pending(&self) -> bool {
        !self.pending_notifications.is_empty()
    }

    /// Check if an entity is marked as colliding.
    pub fn is_colliding(&self, entity: Entity) -> bool {
        self.colliding_entities.contains(&entity)
    }
}

/// Check for collisions between asteroids and celestial bodies.
///
/// This system runs after physics integration. When a collision is detected:
/// 1. The asteroid is marked as colliding (immediate physics exclusion)
/// 2. A notification is queued
/// 3. The asteroid is despawned
/// 4. The simulation is paused
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
        // Skip if already marked as colliding (prevents re-detection before despawn)
        if collision_state.is_colliding(entity) {
            continue;
        }

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

        // Add to collision state FIRST (provides immediate physics exclusion)
        collision_state.push_collision(entity, event.clone());

        // Destroy the asteroid (deferred until end of frame)
        commands.entity(entity).despawn();
        integrator_states.remove(entity);

        // Clear selection if this asteroid was selected
        if selected.body == Some(SelectableBody::Asteroid(entity)) {
            selected.body = None;
        }

        // Fire event for any other listeners
        collision_events.send(event);

        // Pause simulation (will pause on first collision, additional collisions
        // in the same frame are still processed and destroyed)
        sim_time.paused = true;
    }
}

/// Plugin providing collision detection for asteroids.
///
/// Note: Actual collision detection is now performed inside `physics_step`
/// to ensure correct timing (asteroid and celestial body positions synchronized).
/// This plugin only registers the event type and collision state resource.
pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CollisionEvent>()
            .insert_resource(CollisionState::default());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(name: &str) -> CollisionEvent {
        CollisionEvent {
            asteroid_name: name.to_string(),
            body_hit: CelestialBodyId::Sun,
            impact_position: DVec2::ZERO,
            impact_velocity: DVec2::new(1000.0, 0.0),
            time: 0.0,
        }
    }

    #[test]
    fn test_collision_state_push_pop_fifo() {
        let mut state = CollisionState::default();

        let event1 = make_event("Asteroid-1");
        let event2 = make_event("Asteroid-2");
        let event3 = make_event("Asteroid-3");

        state.push_collision(Entity::from_raw(1), event1);
        state.push_collision(Entity::from_raw(2), event2);
        state.push_collision(Entity::from_raw(3), event3);

        // Should have 3 pending
        assert!(state.has_pending());
        assert_eq!(state.pending_notifications.len(), 3);

        // Pop in FIFO order
        let popped1 = state.pop_notification().unwrap();
        assert_eq!(popped1.asteroid_name, "Asteroid-1");

        let popped2 = state.pop_notification().unwrap();
        assert_eq!(popped2.asteroid_name, "Asteroid-2");

        let popped3 = state.pop_notification().unwrap();
        assert_eq!(popped3.asteroid_name, "Asteroid-3");

        // Queue should be empty now
        assert!(!state.has_pending());
        assert!(state.pop_notification().is_none());
    }

    #[test]
    fn test_collision_state_colliding_entities() {
        let mut state = CollisionState::default();

        let entity1 = Entity::from_raw(1);
        let entity2 = Entity::from_raw(2);
        let entity3 = Entity::from_raw(3);

        // Initially no entities are colliding
        assert!(!state.is_colliding(entity1));
        assert!(!state.is_colliding(entity2));

        // Push collision marks entity as colliding immediately
        state.push_collision(entity1, make_event("A1"));
        assert!(state.is_colliding(entity1));
        assert!(!state.is_colliding(entity2));

        state.push_collision(entity2, make_event("A2"));
        assert!(state.is_colliding(entity1));
        assert!(state.is_colliding(entity2));
        assert!(!state.is_colliding(entity3));
    }

    #[test]
    fn test_collision_state_clear() {
        let mut state = CollisionState::default();

        state.push_collision(Entity::from_raw(1), make_event("A1"));
        state.push_collision(Entity::from_raw(2), make_event("A2"));

        assert!(state.has_pending());
        assert!(state.is_colliding(Entity::from_raw(1)));
        assert!(state.is_colliding(Entity::from_raw(2)));

        state.clear();

        // Everything should be cleared
        assert!(!state.has_pending());
        assert!(!state.is_colliding(Entity::from_raw(1)));
        assert!(!state.is_colliding(Entity::from_raw(2)));
        assert!(state.pending_notifications.is_empty());
        assert!(state.colliding_entities.is_empty());
    }

    #[test]
    fn test_collision_event_impact_speed() {
        let event = CollisionEvent {
            asteroid_name: "Test".to_string(),
            body_hit: CelestialBodyId::Sun,
            impact_position: DVec2::ZERO,
            impact_velocity: DVec2::new(10000.0, 0.0), // 10 km/s in m/s
            time: 0.0,
        };

        assert!((event.impact_speed_km_s() - 10.0).abs() < 0.001);
    }
}
