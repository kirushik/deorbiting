//! Physics simulation for asteroid orbital mechanics.
//!
//! This module provides the physics integration layer using the IAS15
//! high-order adaptive integrator. It runs in Bevy's FixedUpdate schedule
//! to maintain consistent physics timesteps.

mod gravity;
mod integrator;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::math::DVec2;

pub use gravity::compute_acceleration;
pub use integrator::{IAS15Config, IAS15State};

use crate::asteroid::Asteroid;
use crate::collision::CollisionState;
use crate::ephemeris::Ephemeris;
use crate::types::{BodyState, SimulationTime, SECONDS_PER_DAY};

/// Plugin providing physics simulation for asteroids.
///
/// Adds systems for:
/// - Physics integration (IAS15) in FixedUpdate
/// - Integrator state management
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IAS15Config::default())
            .insert_resource(IntegratorStates::default())
            .add_systems(FixedUpdate, physics_step);
    }
}

/// Resource storing IAS15 integrator state for each asteroid entity.
///
/// Using a HashMap allows multiple asteroids to be simulated simultaneously,
/// each with their own integrator state.
#[derive(Resource, Default)]
pub struct IntegratorStates {
    states: HashMap<Entity, IAS15State>,
}

impl IntegratorStates {
    /// Get the integrator state for an entity, if it exists.
    pub fn get(&self, entity: Entity) -> Option<&IAS15State> {
        self.states.get(&entity)
    }

    /// Get mutable integrator state for an entity, if it exists.
    pub fn get_mut(&mut self, entity: Entity) -> Option<&mut IAS15State> {
        self.states.get_mut(&entity)
    }

    /// Remove integrator state for an entity (e.g., when despawned).
    pub fn remove(&mut self, entity: Entity) {
        self.states.remove(&entity);
    }
}

/// Main physics integration system.
///
/// Runs in FixedUpdate to maintain consistent physics timesteps.
/// For each asteroid with a BodyState, advances the IAS15 integrator
/// to cover the elapsed simulation time.
///
/// Entities marked as colliding are skipped to prevent them from moving
/// after a collision is detected but before the deferred despawn executes.
fn physics_step(
    mut asteroids: Query<(Entity, &mut BodyState), With<Asteroid>>,
    mut integrator_states: ResMut<IntegratorStates>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    config: Res<IAS15Config>,
    time: Res<Time>,
    collision_state: Res<CollisionState>,
) {
    // Skip if simulation is paused
    if sim_time.paused {
        return;
    }

    // Calculate target simulation time to advance
    // FixedUpdate delta * time_scale * SECONDS_PER_DAY (since scale is sim-days per real-second)
    let target_dt = time.delta_secs_f64() * sim_time.scale * SECONDS_PER_DAY;

    // Skip if no time to advance
    if target_dt <= 0.0 {
        return;
    }

    for (entity, mut body_state) in asteroids.iter_mut() {
        // Skip entities that are colliding (awaiting despawn)
        if collision_state.is_colliding(entity) {
            continue;
        }

        // Get or create integrator state for this asteroid
        let ias15 = integrator_states.states.entry(entity).or_insert_with(|| {
            // Compute initial acceleration at current position
            let initial_acc = compute_acceleration(body_state.pos, sim_time.current, &ephemeris);
            IAS15State::from_body_state(&body_state, initial_acc, &config)
        });

        // Track simulation time for this integration batch
        let mut sim_t = sim_time.current;
        let mut remaining = target_dt;

        // Run IAS15 steps until we've covered the target time delta
        while remaining > 0.0 {
            // Create acceleration function that queries ephemeris at the current sim time
            // plus the relative offset within the step
            let current_sim_t = sim_t;
            let acc_fn = |pos: DVec2, relative_t: f64| -> DVec2 {
                compute_acceleration(pos, current_sim_t + relative_t, &ephemeris)
            };

            // Take one IAS15 step
            ias15.step(acc_fn, &config);

            // Advance tracking
            let step_taken = ias15.dt_last_done;
            remaining -= step_taken;
            sim_t += step_taken;

            // Safety: prevent infinite loops if step size is too small
            if step_taken < 1e-10 {
                warn!("IAS15 step size critically small, breaking integration loop");
                break;
            }
        }

        // Update BodyState with final integrated position and velocity
        body_state.pos = ias15.pos;
        body_state.vel = ias15.vel;
    }
}

/// System to clean up integrator states when asteroids are despawned.
pub fn cleanup_integrator_states(
    mut removed: RemovedComponents<Asteroid>,
    mut integrator_states: ResMut<IntegratorStates>,
) {
    for entity in removed.read() {
        integrator_states.remove(entity);
    }
}
