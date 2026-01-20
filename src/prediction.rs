//! Trajectory prediction for asteroids.
//!
//! This module provides forward simulation of asteroid trajectories,
//! allowing users to see where an asteroid will go based on its current
//! position and velocity.

use bevy::math::DVec2;
use bevy::prelude::*;
use std::time::Instant;

use crate::asteroid::Asteroid;
use crate::camera::{CameraState, RENDER_SCALE};
use crate::continuous::{compute_continuous_thrust, ContinuousDeflector};
use crate::ephemeris::{CelestialBodyId, Ephemeris, GravitySourcesWithId};
use crate::input::DragState;
use crate::physics::{
    compute_acceleration_from_full_sources, compute_adaptive_dt, compute_gravity_full,
    PredictionConfig,
};
use crate::render::z_layers;
use crate::render::SelectedBody;
use crate::types::{BodyState, InputSystemSet, SelectableBody, SimulationTime, AU_TO_METERS};
use crate::ui::velocity_handle::VelocityDragState;

/// Plugin providing trajectory prediction functionality.
pub struct PredictionPlugin;

impl Plugin for PredictionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PredictionSettings>()
            .init_resource::<PredictionState>()
            .init_resource::<TrajectoryCache>()
            .init_resource::<PredictionBudget>()
            .add_systems(
                Update,
                (
                    track_selection_changes,
                    predict_trajectory.run_if(should_run_prediction),
                    draw_trajectory,
                )
                    .chain()
                    // Must run after input systems so we read updated velocity/position
                    .after(InputSystemSet::PositionDrag),
            );
    }
}

/// Configuration for trajectory prediction.
#[derive(Resource)]
pub struct PredictionSettings {
    /// Maximum number of integration steps per prediction.
    pub max_steps: usize,
    /// Maximum simulation time to predict (seconds).
    pub max_time: f64,
    /// How often to recalculate prediction (frames).
    pub update_interval: u32,
    /// Store a point every N steps (trajectory decimation).
    pub point_interval: usize,
}

impl Default for PredictionSettings {
    fn default() -> Self {
        Self {
            max_steps: 200_000,
            max_time: 15.0 * 365.25 * 24.0 * 3600.0, // 15 years in seconds
            update_interval: 10,
            point_interval: 20, // Store every 20th point (reduced density)
        }
    }
}

/// A single point on a predicted trajectory.
#[derive(Clone, Debug)]
pub struct TrajectoryPoint {
    /// Position in meters from barycenter (physics coordinate).
    pub pos: DVec2,
    /// Simulation time in seconds since J2000.
    pub time: f64,
    /// The celestial body whose gravity dominates at this point (None = Sun).
    /// Used for trajectory coloring.
    pub dominant_body: Option<CelestialBodyId>,
}

/// Predicted trajectory path for an asteroid.
#[derive(Component, Default, Clone)]
pub struct TrajectoryPath {
    /// Trajectory points with position, time, and gravitational dominance info.
    pub points: Vec<TrajectoryPoint>,
    /// Whether prediction ended due to collision.
    pub ends_in_collision: bool,
    /// Body that would be hit (if collision predicted).
    pub collision_target: Option<CelestialBodyId>,
    /// Classified trajectory outcome.
    pub outcome: crate::outcome::TrajectoryOutcome,
}

/// State for prediction system.
///
/// Tracks when predictions need recalculation and allows external
/// systems (like velocity handle) to trigger updates.
#[derive(Resource, Default)]
pub struct PredictionState {
    /// Set when prediction needs recalculation.
    needs_update: bool,
    /// Frame counter for periodic updates.
    frame_counter: u32,
    /// Last selected entity (to detect selection changes).
    last_selected: Option<Entity>,
    /// Last simulation time when prediction was calculated.
    last_sim_time: f64,
}

/// Integrator state that can be continued from where we left off.
#[derive(Clone, Debug)]
struct ContinuationState {
    /// Position at end of last prediction
    pos: DVec2,
    /// Velocity at end of last prediction
    vel: DVec2,
    /// Acceleration at end of last prediction
    acc: DVec2,
    /// Current timestep
    dt: f64,
    /// Simulation time at end of last prediction
    sim_t: f64,
    /// Number of steps taken so far
    steps: usize,
}

/// Cache for incremental trajectory extension.
///
/// Instead of recomputing the entire trajectory from scratch on each update,
/// we cache the integrator state and continue from where we left off.
/// This enables efficient computation of very long trajectories (12+ years)
/// while maintaining responsive updates.
#[derive(Resource, Default)]
pub struct TrajectoryCache {
    /// Entity whose trajectory is cached
    entity: Option<Entity>,
    /// Initial state (pos, vel) at prediction start - for invalidation
    initial_pos: DVec2,
    initial_vel: DVec2,
    /// Continuation state for extending trajectory
    continuation: Option<ContinuationState>,
    /// Whether trajectory reached a terminal state (collision or escape)
    is_terminal: bool,
    /// Hash of active deflector configuration (for invalidation)
    deflector_hash: u64,
    /// Simulation time when prediction started (for pruning old points)
    prediction_start_time: f64,
}

impl TrajectoryCache {
    /// Check if the cache is valid for the given entity and state.
    fn is_valid_for(&self, entity: Entity, pos: DVec2, vel: DVec2, deflector_hash: u64) -> bool {
        self.entity == Some(entity)
            && self.initial_pos == pos
            && self.initial_vel == vel
            && self.deflector_hash == deflector_hash
            && !self.is_terminal
    }

    /// Invalidate the cache.
    fn invalidate(&mut self) {
        self.entity = None;
        self.continuation = None;
        self.is_terminal = false;
    }

    /// Store the continuation state for later extension.
    fn store_continuation(
        &mut self,
        entity: Entity,
        initial_pos: DVec2,
        initial_vel: DVec2,
        continuation: ContinuationState,
        deflector_hash: u64,
        start_time: f64,
    ) {
        self.entity = Some(entity);
        self.initial_pos = initial_pos;
        self.initial_vel = initial_vel;
        self.continuation = Some(continuation);
        self.deflector_hash = deflector_hash;
        self.prediction_start_time = start_time;
    }

    /// Mark the trajectory as terminal (collision or escape reached).
    fn mark_terminal(&mut self) {
        self.is_terminal = true;
        self.continuation = None;
    }
}

/// CPU budget management for prediction computation.
///
/// Automatically adapts the number of integration steps per frame based on
/// measured performance. This ensures prediction doesn't cause frame drops
/// on slower hardware while taking full advantage of faster hardware.
#[derive(Resource)]
pub struct PredictionBudget {
    /// Target time budget per prediction update (microseconds).
    /// Default: 5000μs (5ms) = 10% of 60 FPS frame budget.
    pub target_micros: f64,

    /// Exponentially weighted moving average of per-step cost (microseconds).
    /// Updated after each prediction run.
    step_cost_ewma: f64,

    /// EWMA smoothing factor (0..1). Higher = more responsive to recent measurements.
    ewma_alpha: f64,

    /// Computed step budget for next prediction.
    pub steps_budget: usize,

    /// Minimum steps per extension (to make progress even on slow hardware).
    min_steps: usize,

    /// Maximum steps per extension (to bound worst-case latency).
    max_steps: usize,
}

impl Default for PredictionBudget {
    fn default() -> Self {
        Self {
            target_micros: 5000.0, // 5ms = 10% of 60 FPS frame
            step_cost_ewma: 1.0,   // Initial estimate: 1μs per step (will be calibrated)
            ewma_alpha: 0.2,       // Moderate smoothing
            steps_budget: 5000,    // Initial budget
            min_steps: 1000,       // Always make at least this much progress
            max_steps: 20000,      // Cap to prevent long freezes
        }
    }
}

impl PredictionBudget {
    /// Update step cost estimate based on measured performance.
    pub fn update_cost(&mut self, steps_taken: usize, elapsed_micros: f64) {
        if steps_taken > 0 {
            let measured_cost = elapsed_micros / steps_taken as f64;
            self.step_cost_ewma =
                self.ewma_alpha * measured_cost + (1.0 - self.ewma_alpha) * self.step_cost_ewma;

            // Recompute budget based on updated cost estimate
            self.steps_budget = self.compute_budget();
        }
    }

    /// Compute optimal step budget based on target time and cost estimate.
    fn compute_budget(&self) -> usize {
        let optimal = (self.target_micros / self.step_cost_ewma) as usize;
        optimal.clamp(self.min_steps, self.max_steps)
    }

    /// Get the current step budget for extension.
    pub fn get_extension_budget(&self) -> usize {
        self.steps_budget
    }
}

/// Compute a simple hash of deflector configuration for cache invalidation.
fn compute_deflector_hash(deflectors: &[ContinuousDeflector]) -> u64 {
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    let mut hasher = DefaultHasher::new();

    for d in deflectors {
        // Hash relevant fields that affect trajectory
        d.target.hash(&mut hasher);
        d.launch_time.to_bits().hash(&mut hasher);
        std::mem::discriminant(&d.state).hash(&mut hasher);
        std::mem::discriminant(&d.payload).hash(&mut hasher);
    }

    hasher.finish()
}

/// Track selection and time changes to trigger prediction recalculation.
fn track_selection_changes(
    selected: Res<SelectedBody>,
    _sim_time: Res<SimulationTime>,
    mut state: ResMut<PredictionState>,
) {
    let current_entity = match selected.body {
        Some(SelectableBody::Asteroid(entity)) => Some(entity),
        _ => None,
    };

    // Check for selection change
    if current_entity != state.last_selected {
        state.needs_update = true;
        state.last_selected = current_entity;
    }

    // Note: We intentionally do NOT trigger prediction updates based on elapsed
    // simulation time. Frame-based updates (every update_interval frames) are
    // sufficient. Time-based triggers caused once-per-day stutters because the
    // prediction is expensive (up to 50k integration steps).
    //
    // The trajectory visualization doesn't need to be perfectly synchronized with
    // simulation time - the frame-based update interval handles that smoothly.
}

/// Run condition: should we run prediction this frame?
fn should_run_prediction(
    state: Res<PredictionState>,
    settings: Res<PredictionSettings>,
) -> bool {
    state.needs_update || state.frame_counter >= settings.update_interval
}


/// Find the celestial body whose gravity dominates at a given position.
///
/// Compares gravitational acceleration magnitudes (GM/r²) from all bodies.
/// Returns None if the Sun dominates (the default case), or Some(body_id)
/// if a planet or moon's gravity is stronger at that point.
fn find_dominant_body(pos: DVec2, time: f64, ephemeris: &Ephemeris) -> Option<CelestialBodyId> {
    find_dominant_body_from_sources(pos, &ephemeris.get_gravity_sources_with_id(time))
}

/// Find dominant body from pre-fetched gravity sources with IDs.
///
/// More efficient when sources have already been fetched for other calculations.
#[inline]
fn find_dominant_body_from_sources(pos: DVec2, sources: &GravitySourcesWithId) -> Option<CelestialBodyId> {
    let mut max_acc = 0.0_f64;
    let mut dominant = CelestialBodyId::Sun;

    for &(id, body_pos, gm) in sources {
        let delta = body_pos - pos;
        let r_sq = delta.length_squared();

        // Avoid division by zero
        if r_sq < 1.0 {
            return Some(id); // Inside body = that body dominates
        }

        // Gravitational acceleration magnitude: GM/r²
        let acc_mag = gm / r_sq;

        if acc_mag > max_acc {
            max_acc = acc_mag;
            dominant = id;
        }
    }

    // Return None if Sun dominates, Some(id) otherwise
    if dominant == CelestialBodyId::Sun {
        None
    } else {
        Some(dominant)
    }
}

/// Compute trajectory prediction for the selected asteroid.
///
/// Uses Velocity Verlet integrator for all trajectory visualization.
/// This provides consistent, fast results suitable for interactive use.
/// The actual simulation uses IAS15 for high accuracy, but the displayed
/// trajectory preview uses Verlet which is accurate enough for visualization.
///
/// # Incremental Extension
/// When not dragging, uses TrajectoryCache to extend trajectories incrementally.
/// This enables efficient computation of very long trajectories (12+ years)
/// while maintaining responsive updates during drag operations.
#[allow(clippy::too_many_arguments)]
fn predict_trajectory(
    mut asteroids: Query<(Entity, &BodyState, &mut TrajectoryPath), With<Asteroid>>,
    deflectors: Query<(Entity, &ContinuousDeflector)>,
    selected: Res<SelectedBody>,
    ephemeris: Res<Ephemeris>,
    sim_time: Res<SimulationTime>,
    settings: Res<PredictionSettings>,
    mut state: ResMut<PredictionState>,
    mut cache: ResMut<TrajectoryCache>,
    mut budget: ResMut<PredictionBudget>,
    velocity_drag: Res<VelocityDragState>,
    position_drag: Res<DragState>,
    camera: Res<CameraState>,
) {
    // Increment frame counter
    state.frame_counter += 1;

    // Get selected asteroid entity
    let Some(SelectableBody::Asteroid(selected_entity)) = selected.body else {
        cache.invalidate();
        return;
    };

    // Find the selected asteroid
    let Ok((_, body_state, mut trajectory)) = asteroids.get_mut(selected_entity) else {
        cache.invalidate();
        return;
    };

    // Skip if velocity is essentially zero
    if body_state.vel.length() < 1.0 {
        trajectory.points.clear();
        cache.invalidate();
        state.needs_update = false;
        state.frame_counter = 0;
        state.last_sim_time = sim_time.current;
        return;
    }

    // Check if we're in interactive drag mode (either position or velocity)
    let is_dragging = velocity_drag.dragging || position_drag.dragging.is_some();

    // Collect deflector info for this asteroid
    let deflector_snapshot: Vec<ContinuousDeflector> = deflectors
        .iter()
        .filter(|(_, d)| d.target == selected_entity)
        .map(|(_, d)| d.clone())
        .collect();

    let deflector_hash = compute_deflector_hash(&deflector_snapshot);

    // Determine if we can use the cache for incremental extension
    let can_extend = !is_dragging
        && cache.is_valid_for(selected_entity, body_state.pos, body_state.vel, deflector_hash)
        && cache.continuation.is_some();

    // Get step budget from adaptive system
    let step_budget = budget.get_extension_budget();

    if can_extend {
        // Incremental extension: prune old points and continue from cached state
        prune_old_points(&mut trajectory, sim_time.current);

        // Get the continuation state
        let continuation = cache.continuation.as_ref().unwrap().clone();
        let steps_before = continuation.steps;

        // Time the prediction
        let start_time = Instant::now();

        // Extend trajectory from where we left off
        let result = predict_with_verlet_continue(
            selected_entity,
            body_state,
            &ephemeris,
            &settings,
            &mut trajectory,
            camera.zoom,
            &deflector_snapshot,
            continuation,
            step_budget,
        );

        // Update budget with measured performance
        let elapsed_micros = start_time.elapsed().as_micros() as f64;
        let steps_taken = result
            .continuation
            .as_ref()
            .map(|c| c.steps - steps_before)
            .unwrap_or(0);
        budget.update_cost(steps_taken, elapsed_micros);

        // Update cache with new continuation state
        if let Some(new_continuation) = result.continuation {
            cache.continuation = Some(new_continuation);
        }
        if result.is_terminal {
            cache.mark_terminal();
        }
    } else {
        // Full recomputation
        trajectory.points.clear();
        trajectory.ends_in_collision = false;
        trajectory.collision_target = None;

        // Store starting point with dominant body calculation (for coloring)
        let start_dominant = find_dominant_body(body_state.pos, sim_time.current, &ephemeris);
        trajectory.points.push(TrajectoryPoint {
            pos: body_state.pos,
            time: sim_time.current,
            dominant_body: start_dominant,
        });

        // Time the prediction
        let start_time = Instant::now();

        // Run prediction (either dragging mode or full recompute)
        let result = predict_with_verlet_full(
            selected_entity,
            body_state,
            &ephemeris,
            sim_time.current,
            &settings,
            &mut trajectory,
            is_dragging,
            camera.zoom,
            &deflector_snapshot,
        );

        // Update budget with measured performance
        let elapsed_micros = start_time.elapsed().as_micros() as f64;
        let steps_taken = result
            .continuation
            .as_ref()
            .map(|c| c.steps)
            .unwrap_or(0);
        budget.update_cost(steps_taken, elapsed_micros);

        // Update cache if not dragging
        if !is_dragging {
            if let Some(continuation) = result.continuation {
                cache.store_continuation(
                    selected_entity,
                    body_state.pos,
                    body_state.vel,
                    continuation,
                    deflector_hash,
                    sim_time.current,
                );
            }
            if result.is_terminal {
                cache.mark_terminal();
            }
        } else {
            cache.invalidate();
        }
    }

    // Compute trajectory outcome
    let prediction_time_span = trajectory
        .points
        .last()
        .map(|p| p.time - sim_time.current)
        .unwrap_or(0.0);

    let (final_pos, final_vel) = trajectory
        .points
        .last()
        .map(|p| (p.pos, body_state.vel)) // Approximate final velocity
        .unwrap_or((body_state.pos, body_state.vel));

    let impact_velocity = if trajectory.ends_in_collision {
        Some(body_state.vel.length()) // Approximate
    } else {
        None
    };

    trajectory.outcome = crate::outcome::detect_outcome(
        body_state.pos,
        body_state.vel,
        trajectory.ends_in_collision,
        trajectory.collision_target,
        final_pos,
        final_vel,
        prediction_time_span,
        impact_velocity,
    );

    // Mark prediction as up-to-date
    state.needs_update = false;
    state.frame_counter = 0;
    state.last_sim_time = sim_time.current;
}

/// Prune trajectory points that are in the past (before current simulation time).
fn prune_old_points(trajectory: &mut TrajectoryPath, current_time: f64) {
    // Keep some buffer (1 day) to avoid visual popping
    let cutoff = current_time - 86400.0;

    // Find first point that's after cutoff
    let keep_from = trajectory
        .points
        .iter()
        .position(|p| p.time > cutoff)
        .unwrap_or(0);

    if keep_from > 0 {
        trajectory.points.drain(0..keep_from);
    }
}

/// Result of trajectory prediction including continuation state.
struct PredictionResult {
    /// Continuation state for incremental extension (None if terminal)
    continuation: Option<ContinuationState>,
    /// Whether trajectory reached a terminal state (collision or escape)
    is_terminal: bool,
}

/// Predict trajectory using Velocity Verlet integrator with adaptive timestep (full recomputation).
///
/// Uses the same physics-based adaptive timestep as live simulation, ensuring
/// predicted trajectories match actual behavior. Zoom level only affects
/// point storage density (visual smoothness), not integration accuracy.
///
/// Includes continuous thrust from active deflectors in the acceleration
/// calculation for accurate prediction of deflected trajectories.
///
/// # Optimization Note
/// This function uses unified ephemeris queries (`get_gravity_sources_full`) to
/// fetch position, GM, and collision radius in a single pass. This reduces
/// ephemeris interpolations from 24 per step to 8, yielding ~3x speedup.
///
/// Returns a `PredictionResult` with continuation state for incremental extension.
#[allow(clippy::too_many_arguments)]
fn predict_with_verlet_full(
    target_entity: Entity,
    body_state: &BodyState,
    ephemeris: &Ephemeris,
    start_time: f64,
    settings: &PredictionSettings,
    trajectory: &mut TrajectoryPath,
    is_dragging: bool,
    zoom: f32,
    deflectors: &[ContinuousDeflector],
) -> PredictionResult {
    // Use physics-based adaptive timestep with config appropriate for prediction
    let config = if is_dragging {
        PredictionConfig::for_dragging()
    } else {
        PredictionConfig::default()
    };

    // Zoom only affects point storage density (visual smoothness)
    // - Low zoom (zoomed in): store more points for smooth curves
    // - High zoom (zoomed out): store fewer points
    let zoom_scale = (zoom as f64).sqrt().clamp(0.1, 10.0);
    let base_point_interval = if is_dragging { 4 } else { 2 };
    let point_interval = ((base_point_interval as f64 * zoom_scale) as usize).max(1);

    let mut pos = body_state.pos;
    let mut vel = body_state.vel;
    let mut sim_t = start_time;
    let end_t = start_time + settings.max_time;
    let asteroid_mass = body_state.mass;
    let has_deflectors = !deflectors.is_empty();

    // Helper to add continuous thrust to gravity acceleration
    let add_thrust = |gravity_acc: DVec2, pos: DVec2, vel: DVec2, sim_t: f64| -> DVec2 {
        if !has_deflectors {
            gravity_acc
        } else {
            let deflector_refs: Vec<(Entity, &ContinuousDeflector)> = deflectors
                .iter()
                .map(|d| (Entity::PLACEHOLDER, d))
                .collect();
            let thrust_acc = compute_continuous_thrust(
                target_entity,
                pos,
                vel,
                asteroid_mass,
                sim_t,
                &deflector_refs,
            );
            gravity_acc + thrust_acc
        }
    };

    // Initialize with first acceleration using unified query
    let initial_sources = ephemeris.get_gravity_sources_full(sim_t);
    let mut acc = add_thrust(
        compute_acceleration_from_full_sources(pos, &initial_sources),
        pos,
        vel,
        sim_t,
    );
    let mut dt = config.initial_dt;

    let mut step = 0;
    let max_steps = if is_dragging { 1000 } else { settings.max_steps };

    while step < max_steps && sim_t < end_t {
        // Velocity Verlet integration
        // Step 1: Position update
        let pos_new = pos + vel * dt + acc * (0.5 * dt * dt);
        let new_time = sim_t + dt;

        // Step 2: Get all gravity data in ONE ephemeris lookup
        let sources = ephemeris.get_gravity_sources_full(new_time);

        // Step 3: Compute acceleration, dominant body, and collision in ONE pass
        let gravity_result = compute_gravity_full(pos_new, &sources);

        // Add continuous thrust if present
        let vel_approx = vel + acc * dt; // First-order velocity approximation
        let acc_new = add_thrust(gravity_result.acceleration, pos_new, vel_approx, new_time);

        // Step 4: Velocity update
        let vel_new = vel + (acc + acc_new) * (0.5 * dt);

        // Compute adaptive timestep using unified logic
        let dt_new = compute_adaptive_dt(
            acc,
            acc_new,
            dt,
            config.min_dt,
            config.max_dt,
            config.epsilon,
        );

        // Update state
        pos = pos_new;
        vel = vel_new;
        acc = acc_new;
        sim_t = new_time;
        dt = dt_new;
        step += 1;

        // Store points at interval with dominant body info (for coloring)
        // Dominant body was already computed in gravity_result
        if step % point_interval == 0 {
            trajectory.points.push(TrajectoryPoint {
                pos,
                time: sim_t,
                dominant_body: gravity_result.dominant_body,
            });
        }

        // Check collision (already computed in gravity_result)
        if let Some(body_id) = gravity_result.collision {
            trajectory.ends_in_collision = true;
            trajectory.collision_target = Some(body_id);
            // Collision point: the collided body dominates
            trajectory.points.push(TrajectoryPoint {
                pos,
                time: sim_t,
                dominant_body: Some(body_id),
            });
            return PredictionResult {
                continuation: None,
                is_terminal: true,
            };
        }

        // Check escape or crash
        const ESCAPE_DISTANCE: f64 = 100.0 * 1.495978707e11; // 100 AU
        const CRASH_DISTANCE: f64 = 1e9; // ~Sun radius
        if pos.length() > ESCAPE_DISTANCE || pos.length() < CRASH_DISTANCE {
            return PredictionResult {
                continuation: None,
                is_terminal: true,
            };
        }
    }

    // Return continuation state for future extension
    PredictionResult {
        continuation: Some(ContinuationState {
            pos,
            vel,
            acc,
            dt,
            sim_t,
            steps: step,
        }),
        is_terminal: false,
    }
}

/// Continue trajectory prediction from a cached state (incremental extension).
///
/// This function continues where a previous prediction left off, enabling
/// efficient computation of very long trajectories over multiple frames.
#[allow(clippy::too_many_arguments)]
fn predict_with_verlet_continue(
    target_entity: Entity,
    body_state: &BodyState,
    ephemeris: &Ephemeris,
    settings: &PredictionSettings,
    trajectory: &mut TrajectoryPath,
    zoom: f32,
    deflectors: &[ContinuousDeflector],
    continuation: ContinuationState,
    step_budget: usize,
) -> PredictionResult {
    let config = PredictionConfig::default();

    // Zoom only affects point storage density (visual smoothness)
    let zoom_scale = (zoom as f64).sqrt().clamp(0.1, 10.0);
    let point_interval = ((2.0 * zoom_scale) as usize).max(1);

    let mut pos = continuation.pos;
    let mut vel = continuation.vel;
    let mut acc = continuation.acc;
    let mut dt = continuation.dt;
    let mut sim_t = continuation.sim_t;
    let mut step = continuation.steps;
    let end_t = sim_t + settings.max_time;
    let asteroid_mass = body_state.mass;
    let has_deflectors = !deflectors.is_empty();

    // Helper to add continuous thrust to gravity acceleration
    let add_thrust = |gravity_acc: DVec2, pos: DVec2, vel: DVec2, sim_t: f64| -> DVec2 {
        if !has_deflectors {
            gravity_acc
        } else {
            let deflector_refs: Vec<(Entity, &ContinuousDeflector)> = deflectors
                .iter()
                .map(|d| (Entity::PLACEHOLDER, d))
                .collect();
            let thrust_acc = compute_continuous_thrust(
                target_entity,
                pos,
                vel,
                asteroid_mass,
                sim_t,
                &deflector_refs,
            );
            gravity_acc + thrust_acc
        }
    };

    // Use the adaptive step budget from PredictionBudget
    let max_steps_this_frame = step + step_budget;
    let max_total_steps = settings.max_steps;

    while step < max_steps_this_frame && step < max_total_steps && sim_t < end_t {
        // Velocity Verlet integration
        let pos_new = pos + vel * dt + acc * (0.5 * dt * dt);
        let new_time = sim_t + dt;

        // Get all gravity data in ONE ephemeris lookup
        let sources = ephemeris.get_gravity_sources_full(new_time);

        // Compute acceleration, dominant body, and collision in ONE pass
        let gravity_result = compute_gravity_full(pos_new, &sources);

        // Add continuous thrust if present
        let vel_approx = vel + acc * dt;
        let acc_new = add_thrust(gravity_result.acceleration, pos_new, vel_approx, new_time);

        // Velocity update
        let vel_new = vel + (acc + acc_new) * (0.5 * dt);

        // Compute adaptive timestep
        let dt_new = compute_adaptive_dt(
            acc,
            acc_new,
            dt,
            config.min_dt,
            config.max_dt,
            config.epsilon,
        );

        // Update state
        pos = pos_new;
        vel = vel_new;
        acc = acc_new;
        sim_t = new_time;
        dt = dt_new;
        step += 1;

        // Store points at interval
        if step % point_interval == 0 {
            trajectory.points.push(TrajectoryPoint {
                pos,
                time: sim_t,
                dominant_body: gravity_result.dominant_body,
            });
        }

        // Check collision
        if let Some(body_id) = gravity_result.collision {
            trajectory.ends_in_collision = true;
            trajectory.collision_target = Some(body_id);
            trajectory.points.push(TrajectoryPoint {
                pos,
                time: sim_t,
                dominant_body: Some(body_id),
            });
            return PredictionResult {
                continuation: None,
                is_terminal: true,
            };
        }

        // Check escape or crash
        const ESCAPE_DISTANCE: f64 = 100.0 * 1.495978707e11; // 100 AU
        const CRASH_DISTANCE: f64 = 1e9; // ~Sun radius
        if pos.length() > ESCAPE_DISTANCE || pos.length() < CRASH_DISTANCE {
            return PredictionResult {
                continuation: None,
                is_terminal: true,
            };
        }
    }

    // Return continuation state for future extension
    PredictionResult {
        continuation: Some(ContinuationState {
            pos,
            vel,
            acc,
            dt,
            sim_t,
            steps: step,
        }),
        is_terminal: false,
    }
}

/// Draw trajectory using Bevy gizmos.
///
/// Renders trajectory at true physics positions (no distortion).
/// For escape trajectories, applies additional distance-based fading.
fn draw_trajectory(
    trajectories: Query<(Entity, &TrajectoryPath), With<Asteroid>>,
    selected: Res<SelectedBody>,
    mut gizmos: Gizmos,
) {
    // Get selected asteroid entity
    let Some(SelectableBody::Asteroid(selected_entity)) = selected.body else {
        return;
    };

    // Get trajectory for selected asteroid
    let Ok((_, trajectory)) = trajectories.get(selected_entity) else {
        return;
    };

    // Need at least 2 points to draw lines
    if trajectory.points.len() < 2 {
        return;
    }

    let total_points = trajectory.points.len();
    let mut prev_render_pos: Option<Vec3> = None;

    // For escape trajectories, calculate starting position for distance-based fade
    let is_escape = matches!(trajectory.outcome, crate::outcome::TrajectoryOutcome::Escape { .. });
    let start_pos = if is_escape && !trajectory.points.is_empty() {
        Some(trajectory.points[0].pos)
    } else {
        None
    };

    // Max fade distance: 30 AU for escape trajectories
    const MAX_FADE_DISTANCE: f64 = 30.0 * AU_TO_METERS;

    for (i, point) in trajectory.points.iter().enumerate() {
        // Render at true physics position (no distortion)
        let render_pos = Vec3::new(
            (point.pos.x * RENDER_SCALE) as f32,
            (point.pos.y * RENDER_SCALE) as f32,
            z_layers::TRAJECTORY,
        );

        // Draw line segment from previous point
        if let Some(prev) = prev_render_pos {
            let t_normalized = i as f32 / total_points as f32;
            let mut color = trajectory_color(
                t_normalized,
                trajectory.ends_in_collision,
                point.dominant_body,
            );

            // Apply additional distance-based fade for escape trajectories
            if let Some(start) = start_pos {
                let distance = (point.pos - start).length();
                let distance_fade = 1.0 - (distance / MAX_FADE_DISTANCE).min(1.0);
                // Square the fade for more dramatic effect at large distances
                let distance_alpha = (distance_fade * distance_fade) as f32;

                // Multiply existing alpha by distance-based fade
                let current_alpha = color.alpha();
                color = color.with_alpha(current_alpha * distance_alpha.max(0.05));
            }

            gizmos.line(prev, render_pos, color);
        }

        prev_render_pos = Some(render_pos);
    }
}

/// Get the characteristic color for a celestial body.
fn body_color(body_id: CelestialBodyId) -> (f32, f32, f32) {
    match body_id {
        CelestialBodyId::Sun => (0.0, 0.85, 1.0),        // Cyan (default trajectory color)
        CelestialBodyId::Mercury => (0.7, 0.7, 0.7),     // Gray
        CelestialBodyId::Venus => (1.0, 0.9, 0.6),       // Yellow
        CelestialBodyId::Earth => (0.3, 0.5, 1.0),       // Blue
        CelestialBodyId::Mars => (1.0, 0.4, 0.3),        // Red
        CelestialBodyId::Jupiter => (1.0, 0.7, 0.3),     // Orange
        CelestialBodyId::Saturn => (0.9, 0.8, 0.5),      // Gold
        CelestialBodyId::Uranus => (0.5, 0.9, 0.8),      // Cyan-green
        CelestialBodyId::Neptune => (0.3, 0.4, 1.0),     // Deep blue
        // Moons inherit parent color (simplified)
        CelestialBodyId::Moon => (0.3, 0.5, 1.0),        // Blue (like Earth)
        CelestialBodyId::Phobos | CelestialBodyId::Deimos => (1.0, 0.4, 0.3), // Red (like Mars)
        CelestialBodyId::Io
        | CelestialBodyId::Europa
        | CelestialBodyId::Ganymede
        | CelestialBodyId::Callisto => (1.0, 0.7, 0.3),  // Orange (like Jupiter)
        CelestialBodyId::Titan | CelestialBodyId::Enceladus => (0.9, 0.8, 0.5), // Gold (like Saturn)
    }
}

/// Calculate color for trajectory segment based on position along path and dominant body.
fn trajectory_color(
    t_normalized: f32,
    ends_in_collision: bool,
    dominant_body: Option<CelestialBodyId>,
) -> Color {
    // Alpha fades from 1.0 to 0.2 along trajectory
    let alpha = 1.0 - t_normalized * 0.8;

    if ends_in_collision {
        // Collision trajectory: red throughout, intensifying near collision
        // Start orange-red, transition to bright red near collision
        let intensity = 0.6 + t_normalized * 0.4; // 0.6 → 1.0
        let green = 0.3 * (1.0 - t_normalized); // 0.3 → 0.0
        Color::srgba(intensity, green, 0.1, alpha.max(0.5)) // Keep more visible
    } else if let Some(body_id) = dominant_body {
        // Color based on dominant body
        let (r, g, b) = body_color(body_id);
        Color::srgba(r, g, b, alpha)
    } else {
        // Sun-dominated: cyan/teal (default)
        Color::srgba(0.0, 0.85, 1.0, alpha)
    }
}

/// Mark prediction as needing update.
/// Call this when velocity is changed (e.g., from velocity handle drag).
pub fn mark_prediction_dirty(state: &mut PredictionState) {
    state.needs_update = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_color_fades_with_distance() {
        // Sun-dominated (None) trajectory
        let start_color = trajectory_color(0.0, false, None);
        let end_color = trajectory_color(1.0, false, None);

        // Start should be more opaque (higher alpha)
        // Both are cyan colors
        let Color::Srgba(start) = start_color else {
            panic!("Expected Srgba color");
        };
        let Color::Srgba(end) = end_color else {
            panic!("Expected Srgba color");
        };

        assert!(start.alpha > end.alpha, "Color should fade along trajectory");
        assert!(start.alpha > 0.9, "Start should be nearly opaque");
        assert!(end.alpha < 0.3, "End should be mostly transparent");
    }

    #[test]
    fn test_trajectory_color_red_at_collision() {
        let near_collision = trajectory_color(0.95, true, None);
        let normal = trajectory_color(0.95, false, None);

        let Color::Srgba(collision_color) = near_collision else {
            panic!("Expected Srgba color");
        };
        let Color::Srgba(normal_color) = normal else {
            panic!("Expected Srgba color");
        };

        // Collision should be red (high R, low G/B)
        assert!(collision_color.red > 0.8, "Collision should be red");
        assert!(collision_color.green < 0.5, "Collision should have low green");

        // Normal should be cyan (low R, high G/B)
        assert!(normal_color.red < 0.2, "Normal should have low red");
        assert!(normal_color.blue > 0.8, "Normal should be cyan");
    }

    #[test]
    fn test_trajectory_color_by_dominant_body() {
        // Jupiter-dominated segment should be orange
        let jupiter_color = trajectory_color(0.5, false, Some(CelestialBodyId::Jupiter));
        let Color::Srgba(color) = jupiter_color else {
            panic!("Expected Srgba color");
        };

        // Orange: high R, medium G, low B
        assert!(color.red > 0.8, "Jupiter should be orange (high red)");
        assert!(color.green > 0.5 && color.green < 0.9, "Jupiter should be orange (medium green)");
        assert!(color.blue < 0.5, "Jupiter should be orange (low blue)");

        // Earth-dominated segment should be blue
        let earth_color = trajectory_color(0.5, false, Some(CelestialBodyId::Earth));
        let Color::Srgba(e_color) = earth_color else {
            panic!("Expected Srgba color");
        };

        assert!(e_color.blue > 0.8, "Earth should be blue");
        assert!(e_color.red < 0.5, "Earth should have low red");
    }

    #[test]
    fn test_prediction_settings_defaults() {
        let settings = PredictionSettings::default();

        assert_eq!(settings.max_steps, 200_000);
        // 15 years in seconds ≈ 4.73e8
        assert!((settings.max_time - 15.0 * 365.25 * 24.0 * 3600.0).abs() < 1.0);
        assert!(settings.update_interval > 0);
        assert_eq!(settings.point_interval, 20);
    }
}
