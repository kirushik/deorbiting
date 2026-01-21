//! Numerical integrators for orbital mechanics.
//!
//! This module provides integrators for simulating asteroid trajectories.
//! Currently implements Velocity Verlet (2nd order symplectic) with
//! adaptive timestep control.
//!
//! Future: Implement IAS15 for machine-precision accuracy.

use bevy::math::DVec2;
use bevy::prelude::Resource;

use crate::types::BodyState;

// =============================================================================
// Configuration
// =============================================================================

/// Configuration for the integrator.
#[derive(Resource, Clone, Debug)]
pub struct IAS15Config {
    /// Initial timestep in seconds. Default: 3600 (1 hour).
    pub initial_dt: f64,
    /// Minimum allowed timestep in seconds. Default: 60.
    pub min_dt: f64,
    /// Maximum allowed timestep in seconds. Default: 86400 (1 day).
    pub max_dt: f64,
    /// Error tolerance for adaptive stepping. Default: 1e-9.
    pub epsilon: f64,
    /// Safety factor for timestep changes. Default: 0.25.
    pub safety_factor: f64,
}

impl Default for IAS15Config {
    fn default() -> Self {
        Self {
            initial_dt: 3600.0, // 1 hour
            min_dt: 60.0,       // 1 minute
            max_dt: 86400.0,    // 1 day
            epsilon: 1e-9,
            safety_factor: 0.25,
        }
    }
}

// =============================================================================
// Integrator State (Velocity Verlet with adaptive timestep)
// =============================================================================

/// Integrator state for a single body.
///
/// Uses Velocity Verlet (leapfrog) integration, which is:
/// - 2nd order accurate
/// - Symplectic (conserves phase space volume)
/// - Time-reversible
/// - Excellent for orbital mechanics
///
/// Combined with adaptive timestep based on acceleration changes.
#[derive(Clone, Debug)]
pub struct IAS15State {
    /// Current position (meters).
    pub pos: DVec2,
    /// Current velocity (m/s).
    pub vel: DVec2,
    /// Current acceleration (m/s²).
    acc: DVec2,
    /// Previous acceleration for error estimation.
    acc_prev: DVec2,
    /// Current timestep (seconds).
    pub dt: f64,
    /// Last completed timestep.
    pub dt_last_done: f64,
}

impl IAS15State {
    /// Create a new integrator state from initial conditions.
    pub fn new(pos: DVec2, vel: DVec2, initial_acc: DVec2, config: &IAS15Config) -> Self {
        Self {
            pos,
            vel,
            acc: initial_acc,
            acc_prev: initial_acc,
            dt: config.initial_dt,
            dt_last_done: config.initial_dt,
        }
    }

    /// Create from a BodyState component.
    pub fn from_body_state(state: &BodyState, initial_acc: DVec2, config: &IAS15Config) -> Self {
        Self::new(state.pos, state.vel, initial_acc, config)
    }

    /// Perform one Velocity Verlet integration step with adaptive timestep.
    ///
    /// The acceleration function takes (position, relative_time) and returns acceleration.
    ///
    /// Returns `true` if step succeeded.
    pub fn step<F>(&mut self, acceleration_fn: F, config: &IAS15Config) -> bool
    where
        F: Fn(DVec2, f64) -> DVec2,
    {
        let dt = self.dt;

        // Velocity Verlet step:
        // 1. x_new = x + v*dt + 0.5*a*dt²
        // 2. a_new = acceleration(x_new)
        // 3. v_new = v + 0.5*(a + a_new)*dt

        // Step 1: Position update
        let pos_new = self.pos + self.vel * dt + self.acc * (0.5 * dt * dt);

        // Step 2: Compute new acceleration
        let acc_new = acceleration_fn(pos_new, dt);

        // Step 3: Velocity update (using average of old and new acceleration)
        let vel_new = self.vel + (self.acc + acc_new) * (0.5 * dt);

        // Improved error estimation based on local truncation error
        //
        // For Velocity Verlet, local truncation error is O(dt³) and proportional to
        // the third derivative of position (jerk rate). We estimate this using the
        // second central difference of acceleration:
        //
        //   d²a/dt² ≈ (acc_new - 2*acc + acc_prev) / dt²
        //
        // The position error scales as:
        //   δx ≈ (1/12) * dt² * |acc_new - 2*acc + acc_prev|
        //
        // We normalize by position magnitude to get dimensionless error.
        let acc_second_diff = acc_new - self.acc * 2.0 + self.acc_prev;
        let position_error = (1.0 / 12.0) * dt * dt * acc_second_diff.length();

        // Normalize by characteristic length scale (position magnitude or minimum threshold)
        let pos_scale = pos_new.length().max(1e6); // Min 1000 km to avoid issues near origin
        let relative_error = position_error / pos_scale;

        // Compute new timestep based on error
        let dt_new = self.compute_new_timestep(relative_error, config);

        // Accept step
        self.acc_prev = self.acc;
        self.pos = pos_new;
        self.vel = vel_new;
        self.acc = acc_new;
        self.dt_last_done = dt;
        self.dt = dt_new;

        true
    }

    /// Compute new timestep based on error estimate.
    ///
    /// For Velocity Verlet, local truncation error is O(dt³), so to maintain
    /// constant error, we adjust dt by (epsilon/error)^(1/3).
    fn compute_new_timestep(&self, error: f64, config: &IAS15Config) -> f64 {
        if error < 1e-15 {
            // Error essentially zero - increase timestep gradually
            return (self.dt * 1.5).min(config.max_dt);
        }

        // For Verlet, local truncation error ~ dt³
        // To achieve target error epsilon: dt_new = dt * (epsilon/error)^(1/3)
        let ratio = (config.epsilon / error).powf(1.0 / 3.0);

        // Apply conservative clamping to prevent extreme changes
        let ratio_clamped = ratio.clamp(0.5, 2.0);

        (self.dt * ratio_clamped).clamp(config.min_dt, config.max_dt)
    }
}

/// Configuration for trajectory prediction.
///
/// Uses looser tolerances than live physics for faster computation
/// while maintaining visual accuracy.
#[derive(Clone, Debug)]
pub struct PredictionConfig {
    /// Initial timestep in seconds.
    pub initial_dt: f64,
    /// Minimum allowed timestep in seconds.
    pub min_dt: f64,
    /// Maximum allowed timestep in seconds.
    pub max_dt: f64,
    /// Error tolerance (looser than live physics).
    pub epsilon: f64,
}

impl Default for PredictionConfig {
    fn default() -> Self {
        Self {
            initial_dt: 3600.0,    // 1 hour
            min_dt: 600.0,         // 10 minutes (coarser than live physics)
            max_dt: 86400.0 * 2.0, // 2 days (coarser than live physics)
            epsilon: 1e-6,         // Looser tolerance for faster prediction
        }
    }
}

impl PredictionConfig {
    /// Create config for interactive dragging (coarser for responsiveness).
    pub fn for_dragging() -> Self {
        Self {
            initial_dt: 7200.0,    // 2 hours
            min_dt: 3600.0,        // 1 hour minimum
            max_dt: 86400.0 * 4.0, // 4 days
            epsilon: 1e-4,         // Very loose for fast feedback
        }
    }
}

/// Compute adaptive timestep based on acceleration change.
///
/// This is the unified timestep logic used by both live physics and prediction.
/// Uses the relative change in acceleration as an error proxy.
///
/// For Velocity Verlet with local truncation error O(dt³), we adjust dt by
/// (epsilon/error)^(1/3) to maintain target error.
///
/// # Arguments
/// * `acc_old` - Acceleration at start of step
/// * `acc_new` - Acceleration at end of step
/// * `current_dt` - Current timestep
/// * `min_dt` - Minimum allowed timestep
/// * `max_dt` - Maximum allowed timestep
/// * `epsilon` - Error tolerance
///
/// # Returns
/// The new timestep value.
pub fn compute_adaptive_dt(
    acc_old: DVec2,
    acc_new: DVec2,
    current_dt: f64,
    min_dt: f64,
    max_dt: f64,
    epsilon: f64,
) -> f64 {
    // Estimate error from acceleration change
    // This approximates the jerk-related error term in Velocity Verlet
    let acc_change = (acc_new - acc_old).length();

    // Normalize by acceleration scale to get dimensionless error
    let acc_scale = acc_old.length().max(acc_new.length()).max(1e-10);
    let relative_error = acc_change / acc_scale;

    if relative_error < 1e-15 {
        // Error essentially zero - increase timestep gradually
        return (current_dt * 1.5).min(max_dt);
    }

    // For Verlet, local truncation error ~ dt³
    // To achieve target error epsilon: dt_new = dt * (epsilon/error)^(1/3)
    let ratio = (epsilon / relative_error).powf(1.0 / 3.0);

    // Clamp ratio to prevent extreme changes
    let ratio_clamped = ratio.clamp(0.5, 2.0);

    (current_dt * ratio_clamped).clamp(min_dt, max_dt)
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    // Physical constants for tests
    const G: f64 = 6.67430e-11;
    const SUN_MASS: f64 = 1.989e30;
    const AU: f64 = 1.495978707e11;

    /// Simple two-body (Sun-asteroid) acceleration for testing.
    fn two_body_acc(pos: DVec2, _time: f64) -> DVec2 {
        let r_sq = pos.length_squared();
        if r_sq < 1.0 {
            return DVec2::ZERO;
        }
        let r = r_sq.sqrt();
        -pos * (G * SUN_MASS / (r_sq * r))
    }

    #[test]
    fn test_circular_orbit_one_period() {
        // Circular orbit at 1 AU
        let r = AU;
        let v = (G * SUN_MASS / r).sqrt();

        let pos = DVec2::new(r, 0.0);
        let vel = DVec2::new(0.0, v);
        let acc = two_body_acc(pos, 0.0);

        let config = IAS15Config::default();
        let mut state = IAS15State::new(pos, vel, acc, &config);

        // Integrate for 1 orbital period (~365.25 days)
        let period = 2.0 * PI * (r.powi(3) / (G * SUN_MASS)).sqrt();
        let mut t = 0.0;

        while t < period {
            state.step(two_body_acc, &config);
            t += state.dt_last_done;
        }

        // Check final position is close to initial (within 1%)
        let final_r = state.pos.length();
        let error = (final_r - r).abs() / r;
        assert!(
            error < 0.01,
            "Radius error after 1 orbit: {:.4}%",
            error * 100.0
        );

        // Check we're back near starting position
        let pos_error = (state.pos - pos).length() / r;
        assert!(
            pos_error < 0.05,
            "Position error after 1 orbit: {:.4}%",
            pos_error * 100.0
        );
    }

    #[test]
    fn test_energy_conservation_100_orbits() {
        // Elliptical orbit with e=0.3
        let r_perihelion = AU;
        let e = 0.3;
        let a = r_perihelion / (1.0 - e); // Semi-major axis

        // Velocity at perihelion: v² = GM * (2/r - 1/a)
        let v = ((G * SUN_MASS) * (2.0 / r_perihelion - 1.0 / a)).sqrt();

        let pos = DVec2::new(r_perihelion, 0.0);
        let vel = DVec2::new(0.0, v);
        let acc = two_body_acc(pos, 0.0);

        // Initial specific orbital energy: E = v²/2 - GM/r
        let energy_initial = 0.5 * vel.length_squared() - G * SUN_MASS / pos.length();

        let config = IAS15Config::default();
        let mut state = IAS15State::new(pos, vel, acc, &config);

        // Integrate for 100 orbits
        let period = 2.0 * PI * (a.powi(3) / (G * SUN_MASS)).sqrt();
        let mut t = 0.0;
        let target_time = 100.0 * period;

        while t < target_time {
            state.step(two_body_acc, &config);
            t += state.dt_last_done;
        }

        // Final energy
        let energy_final = 0.5 * state.vel.length_squared() - G * SUN_MASS / state.pos.length();

        // Velocity Verlet is symplectic, so energy should be well-conserved
        // Allow 1e-4 relative error for 100 orbits with adaptive timestep
        let relative_error = (energy_final - energy_initial).abs() / energy_initial.abs();
        assert!(
            relative_error < 1e-4,
            "Energy error after 100 orbits: {:.2e} (should be < 1e-4)",
            relative_error
        );
    }

    #[test]
    fn test_orbital_period_accuracy() {
        // Circular orbit at 1 AU - should have ~365.25 day period
        let r = AU;
        let v = (G * SUN_MASS / r).sqrt();

        let pos = DVec2::new(r, 0.0);
        let vel = DVec2::new(0.0, v);
        let acc = two_body_acc(pos, 0.0);

        let config = IAS15Config::default();
        let mut state = IAS15State::new(pos, vel, acc, &config);

        // Find when y crosses zero going positive (completed orbit)
        let mut prev_y = 0.0_f64;
        let mut orbit_time = 0.0;
        let max_time = 400.0 * 86400.0; // 400 days max

        loop {
            state.step(two_body_acc, &config);
            orbit_time += state.dt_last_done;

            // Detect zero crossing in y (positive going)
            if prev_y < 0.0 && state.pos.y >= 0.0 && orbit_time > 100.0 * 86400.0 {
                break;
            }
            prev_y = state.pos.y;

            if orbit_time > max_time {
                panic!("Orbit took longer than 400 days");
            }
        }

        // Expected period: ~365.25 days
        let expected_period = 365.25 * 86400.0;
        let error = (orbit_time - expected_period).abs() / expected_period;

        assert!(
            error < 0.01,
            "Period error: {:.4}% (got {:.2} days, expected ~365.25)",
            error * 100.0,
            orbit_time / 86400.0
        );
    }

    #[test]
    fn test_high_eccentricity_orbit() {
        // Highly elliptical orbit (e=0.9, like a comet)
        let r_perihelion = 0.5 * AU;
        let e = 0.9;
        let a = r_perihelion / (1.0 - e);

        // Velocity at perihelion
        let v = ((G * SUN_MASS) * (2.0 / r_perihelion - 1.0 / a)).sqrt();

        let pos = DVec2::new(r_perihelion, 0.0);
        let vel = DVec2::new(0.0, v);
        let acc = two_body_acc(pos, 0.0);

        // Use smaller timestep for high-e orbit
        let config = IAS15Config {
            initial_dt: 600.0, // 10 minutes
            min_dt: 10.0,
            max_dt: 3600.0, // 1 hour max
            ..Default::default()
        };
        let mut state = IAS15State::new(pos, vel, acc, &config);

        // Integrate for 1 orbit
        let period = 2.0 * PI * (a.powi(3) / (G * SUN_MASS)).sqrt();
        let mut t = 0.0;

        while t < period {
            state.step(two_body_acc, &config);
            t += state.dt_last_done;
        }

        // Should return close to perihelion distance
        let final_r = state.pos.length();
        let error = (final_r - r_perihelion).abs() / r_perihelion;

        // Allow 10% error for high-e orbit (challenging for 2nd order integrator)
        assert!(
            error < 0.10,
            "High-e orbit radius error: {:.2}%",
            error * 100.0
        );
    }

    #[test]
    fn test_timestep_adaptation() {
        // Circular orbit - should stabilize to reasonable timestep
        let r = AU;
        let v = (G * SUN_MASS / r).sqrt();

        let pos = DVec2::new(r, 0.0);
        let vel = DVec2::new(0.0, v);
        let acc = two_body_acc(pos, 0.0);

        let config = IAS15Config::default();
        let mut state = IAS15State::new(pos, vel, acc, &config);

        // Run a few steps
        for _ in 0..20 {
            state.step(two_body_acc, &config);
        }

        // Timestep should have adapted to something reasonable
        // For circular orbit at 1 AU, expect minutes to day
        let dt_minutes = state.dt / 60.0;
        assert!(
            dt_minutes > 0.1 && dt_minutes < 1500.0,
            "Timestep {:.2} minutes seems unreasonable",
            dt_minutes
        );
    }
}
