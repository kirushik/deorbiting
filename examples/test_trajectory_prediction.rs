//! Test trajectory prediction accuracy.
//!
//! This example tests the trajectory prediction algorithm by simulating
//! a circular orbit and verifying the predicted path matches expected results.
//!
//! Run with: cargo run --example test_trajectory_prediction

use bevy::math::DVec2;

/// Physical constants
const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;

/// Sun's standard gravitational parameter (GM) - m³/s²
/// Source: IAU 2015 nominal solar mass parameter
const GM_SUN: f64 = 1.32712440018e20;

/// Compute acceleration at a position from Sun only.
fn compute_acceleration(pos: DVec2) -> DVec2 {
    let r_sq = pos.length_squared();
    if r_sq < 1.0 {
        return DVec2::ZERO;
    }
    let r = r_sq.sqrt();
    -pos * (GM_SUN / (r_sq * r))
}

/// Velocity Verlet integrator state (simplified from IAS15State)
struct IntegratorState {
    pos: DVec2,
    vel: DVec2,
    acc: DVec2,
    dt: f64,
}

impl IntegratorState {
    fn new(pos: DVec2, vel: DVec2, dt: f64) -> Self {
        let acc = compute_acceleration(pos);
        Self { pos, vel, acc, dt }
    }

    fn step(&mut self) {
        // Velocity Verlet integration
        self.pos += self.vel * self.dt + self.acc * (0.5 * self.dt * self.dt);
        let acc_new = compute_acceleration(self.pos);
        self.vel += (self.acc + acc_new) * (0.5 * self.dt);
        self.acc = acc_new;
    }
}

/// Trajectory prediction result
struct TrajectoryPrediction {
    points: Vec<(DVec2, f64)>,
    ends_in_collision: bool,
}

/// Predict trajectory for given initial conditions
fn predict_trajectory(
    initial_pos: DVec2,
    initial_vel: DVec2,
    max_time: f64,
    dt: f64,
    point_interval: usize,
) -> TrajectoryPrediction {
    let mut integrator = IntegratorState::new(initial_pos, initial_vel, dt);
    let mut points = vec![(initial_pos, 0.0)];
    let mut sim_t = 0.0;
    let mut step = 0;

    while sim_t < max_time {
        integrator.step();
        sim_t += dt;
        step += 1;

        // Store point at interval
        if step % point_interval == 0 {
            points.push((integrator.pos, sim_t));
        }

        // Check if crashed into sun (r < 1e9 m)
        if integrator.pos.length() < 1e9 {
            return TrajectoryPrediction {
                points,
                ends_in_collision: true,
            };
        }

        // Check if escaped (r > 100 AU)
        if integrator.pos.length() > 100.0 * AU_TO_METERS {
            break;
        }
    }

    TrajectoryPrediction {
        points,
        ends_in_collision: false,
    }
}

fn main() {
    println!("=== Trajectory Prediction Test ===\n");

    // Test 1: Circular orbit should return to starting point
    test_circular_orbit_prediction();

    // Test 2: Prediction should have reasonable point density
    test_prediction_point_count();

    // Test 3: High-eccentricity orbit
    test_eccentric_orbit_prediction();

    println!("\n=== All trajectory prediction tests passed! ===");
}

fn test_circular_orbit_prediction() {
    println!("Test 1: Circular orbit prediction closure...");

    let distance = 1.0 * AU_TO_METERS;
    let initial_pos = DVec2::new(distance, 0.0);
    let v_circular = (GM_SUN / distance).sqrt();
    let initial_vel = DVec2::new(0.0, v_circular);

    // Predict for 1 year
    let one_year = 365.25 * SECONDS_PER_DAY;
    let dt = 3600.0; // 1 hour
    let point_interval = 24; // Store every 24 hours

    let trajectory = predict_trajectory(initial_pos, initial_vel, one_year, dt, point_interval);

    // Check final position is close to initial
    let (final_pos, final_time) = trajectory.points.last().unwrap();
    let closure_error = (*final_pos - initial_pos).length() / distance;

    println!(
        "  Initial position: ({:.4}, {:.4}) AU",
        initial_pos.x / AU_TO_METERS,
        initial_pos.y / AU_TO_METERS
    );
    println!(
        "  Final position: ({:.4}, {:.4}) AU after {:.1} days",
        final_pos.x / AU_TO_METERS,
        final_pos.y / AU_TO_METERS,
        final_time / SECONDS_PER_DAY
    );
    println!("  Closure error: {:.4}%", closure_error * 100.0);
    println!("  Points in trajectory: {}", trajectory.points.len());

    assert!(closure_error < 0.01, "Orbit should close within 1%");
    assert!(!trajectory.ends_in_collision, "Should not end in collision");

    println!("  PASSED\n");
}

fn test_prediction_point_count() {
    println!("Test 2: Prediction point density...");

    let distance = 1.0 * AU_TO_METERS;
    let initial_pos = DVec2::new(distance, 0.0);
    let v_circular = (GM_SUN / distance).sqrt();
    let initial_vel = DVec2::new(0.0, v_circular);

    // Predict for 1 year with different intervals
    let one_year = 365.25 * SECONDS_PER_DAY;
    let dt = 3600.0;

    // Store every step
    let dense = predict_trajectory(initial_pos, initial_vel, one_year, dt, 1);
    // Store every 24 hours
    let sparse = predict_trajectory(initial_pos, initial_vel, one_year, dt, 24);
    // Store every 10 steps
    let medium = predict_trajectory(initial_pos, initial_vel, one_year, dt, 10);

    println!("  Dense (every step): {} points", dense.points.len());
    println!("  Medium (every 10 steps): {} points", medium.points.len());
    println!("  Sparse (every 24 hours): {} points", sparse.points.len());

    // Verify decimation works correctly
    let expected_dense = (one_year / dt) as usize + 1;
    let expected_sparse = expected_dense / 24 + 1;

    assert!(
        dense.points.len() > sparse.points.len() * 10,
        "Dense should have ~24x more points than sparse"
    );
    let relative_error =
        (sparse.points.len() as f64 - expected_sparse as f64).abs() / expected_sparse as f64;
    assert!(
        relative_error < 0.1,
        "Sparse should have approximately expected points"
    );

    println!("  PASSED\n");
}

fn test_eccentric_orbit_prediction() {
    println!("Test 3: Eccentric orbit prediction...");

    // Start at perihelion of e=0.5 orbit
    let perihelion = 0.5 * AU_TO_METERS;
    let eccentricity = 0.5;
    let a = perihelion / (1.0 - eccentricity);
    let aphelion = a * (1.0 + eccentricity);

    let initial_pos = DVec2::new(perihelion, 0.0);
    let v_perihelion = (GM_SUN * (2.0 / perihelion - 1.0 / a)).sqrt();
    let initial_vel = DVec2::new(0.0, v_perihelion);

    // Expected period
    let period = 2.0 * std::f64::consts::PI * (a.powi(3) / GM_SUN).sqrt();

    println!("  Perihelion: {:.4} AU", perihelion / AU_TO_METERS);
    println!("  Aphelion: {:.4} AU", aphelion / AU_TO_METERS);
    println!("  Expected period: {:.1} days", period / SECONDS_PER_DAY);

    // Predict for 1.5 periods
    let dt = 600.0; // 10 minutes for better accuracy
    let point_interval = 6; // Every hour

    let trajectory = predict_trajectory(initial_pos, initial_vel, period * 1.5, dt, point_interval);

    // Find min and max distances
    let mut min_r: f64 = f64::MAX;
    let mut max_r: f64 = 0.0;
    for (pos, _) in &trajectory.points {
        let r: f64 = pos.length();
        min_r = min_r.min(r);
        max_r = max_r.max(r);
    }

    let perihelion_error = (min_r - perihelion).abs() / perihelion;
    let aphelion_error = (max_r - aphelion).abs() / aphelion;

    println!(
        "  Measured perihelion: {:.4} AU (error: {:.3}%)",
        min_r / AU_TO_METERS,
        perihelion_error * 100.0
    );
    println!(
        "  Measured aphelion: {:.4} AU (error: {:.3}%)",
        max_r / AU_TO_METERS,
        aphelion_error * 100.0
    );
    println!("  Points in trajectory: {}", trajectory.points.len());

    assert!(perihelion_error < 0.02, "Perihelion should be within 2%");
    assert!(aphelion_error < 0.02, "Aphelion should be within 2%");
    assert!(!trajectory.ends_in_collision, "Should not collide");

    println!("  PASSED\n");
}
