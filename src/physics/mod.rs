//! Physics simulation for asteroid orbital mechanics.
//!
//! This module provides the physics integration layer using the IAS15
//! high-order adaptive integrator. It runs in Bevy's FixedUpdate schedule
//! to maintain consistent physics timesteps.

mod gravity;
mod integrator;

#[cfg(test)]
mod proptest_physics;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy::math::DVec2;

pub use gravity::{
    compute_acceleration, compute_acceleration_from_full_sources, compute_acceleration_from_sources,
    compute_gravity_full, find_closest_body, ClosestBodyInfo, GravityResult,
};
pub use integrator::{IAS15Config, IAS15State, PredictionConfig, compute_adaptive_dt};

use crate::asteroid::{Asteroid, AsteroidName};
use crate::collision::{CollisionEvent, CollisionState, handle_collision_response};
use crate::continuous::{
    compute_continuous_thrust, update_deflector_progress, ContinuousDeflector,
};
use crate::ephemeris::Ephemeris;
use crate::render::SelectedBody;
use crate::types::{BodyState, SimulationTime, SECONDS_PER_DAY};

/// System set for physics simulation, used for ordering with collision detection.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhysicsSet;

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IAS15Config::default())
            .insert_resource(IntegratorStates::default())
            .add_systems(FixedUpdate, physics_step.in_set(PhysicsSet))
            // Clean up integrator states when asteroids are despawned
            .add_systems(PostUpdate, cleanup_integrator_states);
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
/// This system is the authoritative source of simulation time advancement.
/// It updates `sim_time.current` based on actual physics integration.
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
    mut deflectors: Query<(Entity, &mut ContinuousDeflector)>,
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

    // Track whether any collision occurred (for time advancement)
    let mut had_collision = false;

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

            // Cap step size to remaining time to prevent overshoot.
            // This ensures physics time exactly matches simulation clock advancement.
            // Without this cap, IAS15's adaptive timestep could take a step larger
            // than target_dt, causing the asteroid to advance more than the clock.
            let remaining = target_dt - elapsed;
            if ias15.dt > remaining && remaining > config.min_dt {
                ias15.dt = remaining;
            }

            // Collect deflector info snapshot for this asteroid
            // This allows us to include continuous thrust in the acceleration calculation
            let deflector_snapshot: Vec<(Entity, ContinuousDeflector)> = deflectors
                .iter()
                .filter(|(_, d)| d.target == entity && d.is_operating())
                .map(|(e, d)| (e, d.clone()))
                .collect();

            // Create acceleration function that includes both gravity and continuous thrust
            let current_sim_t = sim_t;
            let asteroid_mass = body_state.mass;
            let current_vel = ias15.vel; // Capture velocity before closure to avoid borrow conflict
            let acc_fn = |pos: DVec2, relative_t: f64| -> DVec2 {
                // Gravity from celestial bodies
                let gravity_acc = compute_acceleration(pos, current_sim_t + relative_t, &ephemeris);

                // Continuous thrust from active deflectors
                if deflector_snapshot.is_empty() {
                    gravity_acc
                } else {
                    let deflector_refs: Vec<(Entity, &ContinuousDeflector)> = deflector_snapshot
                        .iter()
                        .map(|(e, d)| (*e, d))
                        .collect();
                    let thrust_acc = compute_continuous_thrust(
                        entity,
                        pos,
                        current_vel, // Use captured velocity for direction calculation
                        asteroid_mass,
                        current_sim_t + relative_t,
                        &deflector_refs,
                    );
                    gravity_acc + thrust_acc
                }
            };

            // Take one IAS15 step
            ias15.step(acc_fn, &config);

            // Advance elapsed time (single accumulation from zero)
            let step_taken = ias15.dt_last_done;
            elapsed += step_taken;

            // Update deflector progress (fuel consumption, accumulated delta-v)
            for (deflector_entity, _) in &deflector_snapshot {
                if let Ok((_, mut deflector)) = deflectors.get_mut(*deflector_entity) {
                    update_deflector_progress(&mut deflector, asteroid_mass, ias15.pos, step_taken);
                }
            }

            // Check collision at the correct simulation time (after step)
            // This ensures asteroid position and celestial body positions are synchronized
            let sim_t_after_step = start_time + elapsed;
            if let Some(body_hit) = ephemeris.check_collision(ias15.pos, sim_t_after_step) {
                // Update sim_time.current to the collision time BEFORE handling
                // This ensures the displayed time matches the collision time
                sim_time.current = sim_t_after_step;

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

                // Mark collision occurred (sim_time was already updated above)
                had_collision = true;
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

    // Advance simulation time by target_dt (step cap ensures no overshoot)
    // Skip if collision already set sim_time to collision point
    if !sim_time.paused && !had_collision {
        sim_time.current += target_dt;
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
