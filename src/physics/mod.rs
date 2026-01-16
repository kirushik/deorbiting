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

pub use gravity::{compute_acceleration, compute_acceleration_from_sources, find_closest_body, ClosestBodyInfo};
pub use integrator::{IAS15Config, IAS15State, PredictionConfig, compute_adaptive_dt};

use crate::asteroid::{Asteroid, AsteroidName};
use crate::collision::{CollisionEvent, CollisionState, handle_collision_response};
use crate::ephemeris::Ephemeris;
use crate::render::SelectedBody;
use crate::types::{BodyState, SimulationTime, SECONDS_PER_DAY};

/// Plugin providing physics simulation for asteroids.
///
/// Adds systems for:
/// - Physics integration (IAS15) in FixedUpdate
/// - Integrator state management
/// System set for physics simulation, used for ordering with collision detection.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhysicsSet;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IAS15Config::default())
            .insert_resource(IntegratorStates::default())
            .add_systems(FixedUpdate, physics_step.in_set(PhysicsSet));
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
/// Collision detection is performed inside the integration loop at each step
/// to ensure the correct simulation time is used (asteroid and celestial body
/// positions are synchronized).
///
/// Entities marked as colliding are skipped to prevent them from moving
/// after a collision is detected but before the deferred despawn executes.
#[allow(clippy::too_many_arguments)]
fn physics_step(
    mut commands: Commands,
    mut asteroids: Query<(Entity, &AsteroidName, &mut BodyState), With<Asteroid>>,
    mut integrator_states: ResMut<IntegratorStates>,
    ephemeris: Res<Ephemeris>,
    mut sim_time: ResMut<SimulationTime>,
    config: Res<IAS15Config>,
    time: Res<Time>,
    mut collision_state: ResMut<CollisionState>,
    mut collision_events: EventWriter<CollisionEvent>,
    mut selected: ResMut<SelectedBody>,
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

    // Track entities that collided (for deferred integrator state removal)
    let mut collided_entities = Vec::new();

    for (entity, name, mut body_state) in asteroids.iter_mut() {
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
        // Use elapsed time from zero to avoid float accumulation errors
        // sim_t = start_time + elapsed (single addition to large value)
        let start_time = sim_time.current;
        let mut elapsed = 0.0;
        let mut collided = false;

        // Run IAS15 steps until we've covered the target time delta
        while elapsed < target_dt {
            // Compute current simulation time precisely
            let sim_t = start_time + elapsed;

            // Only apply proximity timestep cap when actually near a collision boundary.
            // This prevents unnecessary slowdown when far from planets.
            if let Some(closest) = find_closest_body(ias15.pos, sim_t, &ephemeris) {
                // Only cap timestep when within 3x collision radius (approaching danger zone)
                if closest.distance < closest.collision_radius * 3.0 {
                    let rel_vel = (ias15.vel - closest.body_velocity).length();

                    if rel_vel > 0.0 {
                        // Distance to collision boundary (not center)
                        let dist_to_boundary = (closest.distance - closest.collision_radius).max(1e3);

                        // Cap so we move at most 50% of distance to boundary per step
                        let safety_factor = 0.5;
                        let max_dt_proximity = dist_to_boundary * safety_factor / rel_vel;

                        // Apply the cap (don't go below min_dt)
                        let capped_dt = ias15.dt.min(max_dt_proximity).max(config.min_dt);
                        if capped_dt < ias15.dt {
                            ias15.dt = capped_dt;
                        }
                    }
                }
            }

            // Create acceleration function that queries ephemeris at the current sim time
            // plus the relative offset within the step
            let current_sim_t = sim_t;
            let acc_fn = |pos: DVec2, relative_t: f64| -> DVec2 {
                compute_acceleration(pos, current_sim_t + relative_t, &ephemeris)
            };

            // Take one IAS15 step
            ias15.step(acc_fn, &config);

            // Advance elapsed time (single accumulation from zero)
            let step_taken = ias15.dt_last_done;
            elapsed += step_taken;

            // Check collision at the correct simulation time (after step)
            // This ensures asteroid position and celestial body positions are synchronized
            let sim_t_after_step = start_time + elapsed;
            if let Some(body_hit) = ephemeris.check_collision(ias15.pos, sim_t_after_step) {
                // Delegate collision handling to the collision module
                handle_collision_response(
                    &mut commands,
                    &mut collision_state,
                    &mut collision_events,
                    &mut selected,
                    &mut sim_time,
                    entity,
                    &name.0,
                    body_hit,
                    ias15.pos,
                    ias15.vel,
                    sim_t_after_step,
                );

                // Track for deferred integrator state removal
                collided_entities.push(entity);

                collided = true;
                break;
            }

            // Safety: prevent infinite loops if step size is too small
            if step_taken < 1e-10 {
                warn!("IAS15 step size critically small, breaking integration loop");
                break;
            }
        }

        // Update BodyState with final integrated position and velocity (if not collided)
        if !collided {
            body_state.pos = ias15.pos;
            body_state.vel = ias15.vel;
        }
    }

    // Clean up integrator states for collided entities (deferred to avoid borrow conflict)
    for entity in collided_entities {
        integrator_states.remove(entity);
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
