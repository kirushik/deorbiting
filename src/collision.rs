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

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::ephemeris::data::CelestialBodyId;
use crate::render::SelectedBody;
use crate::types::{SECONDS_PER_DAY, SelectableBody, SimulationTime};

/// Event fired when an asteroid collides with a celestial body.
///
/// This event is sent when collision is detected, allowing UI systems
/// to display impact information. The asteroid is destroyed after the event.
#[derive(Message, Clone, Debug)]
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

/// Handle the response to a detected collision.
///
/// This function is called by the physics integration loop when a collision
/// is detected. It:
/// 1. Creates and logs the collision event
/// 2. Marks the entity as colliding (immediate physics exclusion)
/// 3. Queues the asteroid for despawning
/// 4. Clears selection if the asteroid was selected
/// 5. Fires the collision event for UI listeners
/// 6. Pauses the simulation
///
/// Returns the created collision event.
#[allow(clippy::too_many_arguments)]
pub fn handle_collision_response(
    commands: &mut Commands,
    collision_state: &mut CollisionState,
    collision_events: &mut MessageWriter<'_, CollisionEvent>,
    selected: &mut SelectedBody,
    sim_time: &mut SimulationTime,
    entity: Entity,
    asteroid_name: &str,
    body_hit: CelestialBodyId,
    impact_position: DVec2,
    impact_velocity: DVec2,
    time: f64,
) -> CollisionEvent {
    let event = CollisionEvent {
        asteroid_name: asteroid_name.to_string(),
        body_hit,
        impact_position,
        impact_velocity,
        time,
    };

    info!(
        "IMPACT! {} hit {:?} at {:.2} km/s",
        asteroid_name,
        body_hit,
        event.impact_speed_km_s(),
    );

    // Mark as colliding FIRST (provides immediate physics exclusion)
    collision_state.push_collision(entity, event.clone());

    // Destroy the asteroid (deferred until end of frame)
    commands.entity(entity).despawn();

    // Clear selection if this asteroid was selected
    if selected.body == Some(SelectableBody::Asteroid(entity)) {
        selected.body = None;
    }

    // Fire event for any other listeners
    collision_events.write(event.clone());

    // Pause simulation
    sim_time.paused = true;

    event
}

/// Plugin providing collision detection for asteroids.
///
/// Collision detection is performed inside `physics_step` at each integration
/// sub-step to ensure correct timing (asteroid and celestial body positions
/// synchronized). When a collision is detected, physics calls
/// `handle_collision_response` to process the impact.
///
/// This plugin registers the event type and collision state resource.
pub struct CollisionPlugin;

impl Plugin for CollisionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Messages<CollisionEvent>>()
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

        state.push_collision(Entity::from_bits(1), event1);
        state.push_collision(Entity::from_bits(2), event2);
        state.push_collision(Entity::from_bits(3), event3);

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

        let entity1 = Entity::from_bits(1);
        let entity2 = Entity::from_bits(2);
        let entity3 = Entity::from_bits(3);

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

        state.push_collision(Entity::from_bits(1), make_event("A1"));
        state.push_collision(Entity::from_bits(2), make_event("A2"));

        assert!(state.has_pending());
        assert!(state.is_colliding(Entity::from_bits(1)));
        assert!(state.is_colliding(Entity::from_bits(2)));

        state.clear();

        // Everything should be cleared
        assert!(!state.has_pending());
        assert!(!state.is_colliding(Entity::from_bits(1)));
        assert!(!state.is_colliding(Entity::from_bits(2)));
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

    #[test]
    fn test_collision_event_time_days() {
        let event = CollisionEvent {
            asteroid_name: "Test".to_string(),
            body_hit: CelestialBodyId::Earth,
            impact_position: DVec2::ZERO,
            impact_velocity: DVec2::ZERO,
            time: SECONDS_PER_DAY * 365.25, // 1 year in seconds
        };

        let days = event.time_days();
        assert!((days - 365.25).abs() < 0.001);
    }

    #[test]
    fn test_collision_state_default() {
        let state = CollisionState::default();
        assert!(!state.has_pending());
        assert!(state.pending_notifications.is_empty());
        assert!(state.colliding_entities.is_empty());
    }

    #[test]
    fn test_collision_state_multiple_entities_independent() {
        let mut state = CollisionState::default();

        let e1 = Entity::from_bits(1);
        let e2 = Entity::from_bits(2);
        let e3 = Entity::from_bits(3);

        // Add e1 and e2 as colliding
        state.push_collision(e1, make_event("A1"));
        state.push_collision(e2, make_event("A2"));

        // e3 should not be affected
        assert!(!state.is_colliding(e3));
        assert!(state.is_colliding(e1));
        assert!(state.is_colliding(e2));

        // Pop one notification - entities should still be colliding
        let _ = state.pop_notification();
        assert!(state.is_colliding(e1));
        assert!(state.is_colliding(e2));
    }

    #[test]
    fn test_collision_event_velocity_vector() {
        // Test that velocity vector components are preserved
        let event = CollisionEvent {
            asteroid_name: "Test".to_string(),
            body_hit: CelestialBodyId::Mars,
            impact_position: DVec2::new(1e11, 2e11),
            impact_velocity: DVec2::new(3000.0, 4000.0), // 5 km/s total
            time: 100.0,
        };

        // Velocity magnitude: sqrt(3^2 + 4^2) = 5 km/s
        assert!((event.impact_speed_km_s() - 5.0).abs() < 0.001);
        // Check components preserved
        assert_eq!(event.impact_velocity.x, 3000.0);
        assert_eq!(event.impact_velocity.y, 4000.0);
    }

    #[test]
    fn test_collision_state_pop_returns_none_when_empty() {
        let mut state = CollisionState::default();
        assert!(state.pop_notification().is_none());

        // Add one and pop it
        state.push_collision(Entity::from_bits(1), make_event("A"));
        assert!(state.pop_notification().is_some());

        // Should be empty again
        assert!(state.pop_notification().is_none());
    }

    #[test]
    fn test_collision_event_different_bodies() {
        // Test events for different celestial bodies
        let bodies = [
            CelestialBodyId::Sun,
            CelestialBodyId::Mercury,
            CelestialBodyId::Earth,
            CelestialBodyId::Jupiter,
        ];

        for body in bodies {
            let event = CollisionEvent {
                asteroid_name: format!("Asteroid-{:?}", body),
                body_hit: body,
                impact_position: DVec2::ZERO,
                impact_velocity: DVec2::new(1000.0, 0.0),
                time: 0.0,
            };
            assert_eq!(event.body_hit, body);
        }
    }
}
