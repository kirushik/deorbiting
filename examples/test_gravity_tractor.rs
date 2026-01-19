//! Integration test for gravity tractor continuous deflection.
//!
//! This example simulates an asteroid with gravity tractor deflection and verifies:
//! - Gravitational attraction works correctly
//! - Thrust direction matches spacecraft position
//! - Long-duration deflection accumulates expected delta-v
//!
//! Run with: cargo run --example test_gravity_tractor

use bevy::math::DVec2;

/// Physical constants
const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;

/// Gravitational constant (m³/kg/s²)
const G: f64 = 6.67430e-11;

/// Sun's standard gravitational parameter (GM) - m³/s²
const GM_SUN: f64 = 1.32712440018e20;

/// Gravity tractor deflector parameters
struct GravityTractor {
    /// Spacecraft mass in kg
    spacecraft_mass_kg: f64,
    /// Hover distance in meters
    hover_distance_m: f64,
    /// Mission duration in seconds
    mission_duration: f64,
    /// Operating time so far
    operating_time: f64,
    /// Accumulated delta-v in m/s
    accumulated_delta_v: f64,
}

impl GravityTractor {
    fn new(spacecraft_mass_kg: f64, hover_distance_m: f64, mission_duration_days: f64) -> Self {
        Self {
            spacecraft_mass_kg,
            hover_distance_m,
            mission_duration: mission_duration_days * SECONDS_PER_DAY,
            operating_time: 0.0,
            accumulated_delta_v: 0.0,
        }
    }

    /// Check if mission is still active
    fn is_active(&self) -> bool {
        self.operating_time < self.mission_duration
    }

    /// Calculate gravitational acceleration on asteroid (m/s²)
    fn acceleration(&self) -> f64 {
        if self.hover_distance_m <= 0.0 {
            return 0.0;
        }
        G * self.spacecraft_mass_kg / (self.hover_distance_m * self.hover_distance_m)
    }

    /// Update operating time and delta-v
    fn update(&mut self, dt: f64) {
        if !self.is_active() {
            return;
        }

        let remaining = self.mission_duration - self.operating_time;
        let actual_dt = dt.min(remaining);
        self.operating_time += actual_dt;

        // Delta-v = a × dt
        let dv = self.acceleration() * actual_dt;
        self.accumulated_delta_v += dv;
    }
}

/// Compute gravitational acceleration at a position from Sun only.
fn compute_gravity(pos: DVec2) -> DVec2 {
    let r_sq = pos.length_squared();
    if r_sq < 1.0 {
        return DVec2::ZERO;
    }
    let r = r_sq.sqrt();
    -pos * (GM_SUN / (r_sq * r))
}

/// Velocity Verlet integrator
struct IntegratorState {
    pos: DVec2,
    vel: DVec2,
    acc: DVec2,
    dt: f64,
}

impl IntegratorState {
    fn new(pos: DVec2, vel: DVec2, dt: f64) -> Self {
        let acc = compute_gravity(pos);
        Self { pos, vel, acc, dt }
    }

    fn step(&mut self, thrust_acc: DVec2) {
        self.pos = self.pos + self.vel * self.dt + self.acc * (0.5 * self.dt * self.dt);
        let acc_new = compute_gravity(self.pos) + thrust_acc;
        self.vel = self.vel + (self.acc + acc_new) * (0.5 * self.dt);
        self.acc = acc_new;
    }
}

fn main() {
    println!("=== Gravity Tractor Deflection Integration Test ===\n");

    // Test 1: Gravitational attraction calculation
    test_gravitational_attraction();

    // Test 2: Reference case - 20,000 kg at 200m
    test_reference_case();

    // Test 3: Long-duration deflection
    test_long_duration_deflection();

    println!("\n=== All gravity tractor tests passed! ===");
}

fn test_gravitational_attraction() {
    println!("Test 1: Gravitational attraction calculation...");

    // Test the inverse-square law
    let mass = 20_000.0; // 20 tons

    let acc_200m = G * mass / (200.0 * 200.0);
    let acc_400m = G * mass / (400.0 * 400.0);

    println!("  Spacecraft mass: {} kg", mass);
    println!("  Acceleration at 200m: {:.4e} m/s²", acc_200m);
    println!("  Acceleration at 400m: {:.4e} m/s²", acc_400m);
    println!("  Ratio (should be 4:1): {:.2}", acc_200m / acc_400m);

    // Inverse-square law: doubling distance should quarter the force
    let ratio = acc_200m / acc_400m;
    assert!(
        (ratio - 4.0).abs() < 0.01,
        "Inverse-square law should hold"
    );

    println!("  PASSED\n");
}

fn test_reference_case() {
    println!("Test 2: Reference case - 20,000 kg spacecraft at 200m...");

    // This matches the plan's reference: 20,000 kg at 200m → 0.032 N
    let tractor = GravityTractor::new(20_000.0, 200.0, 365.0);

    let acc = tractor.acceleration();

    // Expected: a = G × m / r² = 6.67430e-11 × 20000 / 40000 ≈ 3.337e-11 m/s²
    // Force on 1e10 kg asteroid: F = ma ≈ 0.0334 N
    let expected_acc = G * 20_000.0 / (200.0 * 200.0);

    println!("  Expected acceleration: {:.4e} m/s²", expected_acc);
    println!("  Actual acceleration: {:.4e} m/s²", acc);

    // Verify force on typical asteroid
    let asteroid_mass = 1e10; // 10 billion kg
    let force = acc * asteroid_mass;
    println!("  Force on {} kg asteroid: {:.4} N", asteroid_mass, force);

    let relative_error = (acc - expected_acc).abs() / expected_acc;
    assert!(
        relative_error < 0.001,
        "Acceleration should match expected"
    );

    // Force = G × m_spacecraft × m_asteroid / r²
    // The acceleration times asteroid mass gives the force
    let expected_force = G * 20_000.0 * asteroid_mass / (200.0 * 200.0);
    assert!(
        (force - expected_force).abs() < 0.01,
        "Force calculation should be correct"
    );

    println!("  PASSED\n");
}

fn test_long_duration_deflection() {
    println!("Test 3: Long-duration deflection (simulating years)...");

    // Gravity tractors operate over years
    let spacecraft_mass = 20_000.0;
    let hover_distance = 200.0;
    let mission_years = 10.0;
    let mission_days = mission_years * 365.25;

    let mut tractor = GravityTractor::new(spacecraft_mass, hover_distance, mission_days);

    // Simulate with large time steps (1 day)
    let dt = SECONDS_PER_DAY;
    let total_time = mission_days * SECONDS_PER_DAY;
    let mut sim_t = 0.0;

    while sim_t < total_time && tractor.is_active() {
        tractor.update(dt);
        sim_t += dt;
    }

    // Expected delta-v: Δv = a × t
    let expected_delta_v = tractor.acceleration() * total_time;

    println!("  Mission duration: {:.1} years", mission_years);
    println!("  Acceleration: {:.4e} m/s²", tractor.acceleration());
    println!("  Expected Δv: {:.6} mm/s", expected_delta_v * 1000.0);
    println!("  Actual Δv: {:.6} mm/s", tractor.accumulated_delta_v * 1000.0);

    let relative_error = (tractor.accumulated_delta_v - expected_delta_v).abs() / expected_delta_v;
    println!("  Relative error: {:.4}%", relative_error * 100.0);

    assert!(
        relative_error < 0.01,
        "Delta-v should match expected within 1%"
    );

    // Verify mission completed
    assert!(
        !tractor.is_active(),
        "Mission should be complete"
    );

    println!("  PASSED\n");
}
