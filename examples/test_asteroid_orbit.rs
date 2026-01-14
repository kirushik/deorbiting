//! Test asteroid orbital mechanics without the full GUI.
//!
//! This example tests the physics integration by simulating an asteroid
//! orbiting the Sun for one year and verifying orbital stability.
//!
//! Run with: cargo run --example test_asteroid_orbit

use bevy::math::DVec2;

// Import from the main crate
// Note: Since this is a binary crate (no lib.rs), we need to duplicate some code
// or refactor to a library. For now, we'll use the essential physics directly.

/// Physical constants
const G: f64 = 6.67430e-11;
const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;

/// Sun's standard gravitational parameter (GM)
const SUN_GM: f64 = 1.32712440018e20; // m³/s²

/// Compute acceleration at a position from Sun only.
fn compute_acceleration_sun_only(pos: DVec2) -> DVec2 {
    let r_sq = pos.length_squared();
    if r_sq < 1.0 {
        return DVec2::ZERO;
    }
    let r = r_sq.sqrt();
    -pos * (SUN_GM / (r_sq * r))
}

/// Simple Velocity Verlet step.
fn verlet_step(
    pos: &mut DVec2,
    vel: &mut DVec2,
    acc: &mut DVec2,
    dt: f64,
) {
    // Position update
    *pos = *pos + *vel * dt + *acc * (0.5 * dt * dt);

    // New acceleration
    let acc_new = compute_acceleration_sun_only(*pos);

    // Velocity update
    *vel = *vel + (*acc + acc_new) * (0.5 * dt);

    *acc = acc_new;
}

fn main() {
    println!("=== Asteroid Orbit Integration Test ===\n");

    // Test 1: Circular orbit at 1.5 AU
    test_circular_orbit(1.5);

    // Test 2: Circular orbit at 1.0 AU (Earth-like)
    test_circular_orbit(1.0);

    // Test 3: Elliptical orbit (e=0.5)
    test_elliptical_orbit(1.0, 0.5);

    println!("\n=== All tests completed successfully! ===");
}

fn test_circular_orbit(distance_au: f64) {
    println!("Testing circular orbit at {:.2} AU...", distance_au);

    let distance = distance_au * AU_TO_METERS;
    let mut pos = DVec2::new(distance, 0.0);

    // Circular orbit velocity: v = sqrt(GM/r)
    let v_circular = (SUN_GM / distance).sqrt();
    let mut vel = DVec2::new(0.0, v_circular);

    let mut acc = compute_acceleration_sun_only(pos);

    // Expected orbital period: T = 2π * sqrt(r³/GM)
    let period = 2.0 * std::f64::consts::PI * (distance.powi(3) / SUN_GM).sqrt();
    let period_days = period / SECONDS_PER_DAY;

    println!("  Initial: r = {:.4} AU, v = {:.2} km/s", distance / AU_TO_METERS, v_circular / 1000.0);
    println!("  Expected period: {:.2} days", period_days);

    // Use fixed timestep for simplicity
    let dt = 3600.0; // 1 hour
    let steps_per_day = (SECONDS_PER_DAY / dt) as usize;
    let total_steps = (period / dt) as usize + steps_per_day;

    // Initial energy
    let e0 = 0.5 * vel.length_squared() - SUN_GM / pos.length();

    let mut t = 0.0;
    let mut min_r = distance;
    let mut max_r = distance;

    for step in 0..total_steps {
        verlet_step(&mut pos, &mut vel, &mut acc, dt);
        t += dt;

        let r = pos.length();
        min_r = min_r.min(r);
        max_r = max_r.max(r);

        // Print progress every 30 days
        if step % (30 * steps_per_day) == 0 && step > 0 {
            println!("    Day {:>6.0}: r = {:.4} AU", t / SECONDS_PER_DAY, r / AU_TO_METERS);
        }
    }

    // Final energy
    let ef = 0.5 * vel.length_squared() - SUN_GM / pos.length();
    let energy_error = (ef - e0).abs() / e0.abs();

    // Check orbital stability
    let r_variation = (max_r - min_r) / distance;

    println!("  Final: r = {:.4} AU after {:.2} days", pos.length() / AU_TO_METERS, t / SECONDS_PER_DAY);
    println!("  Radius variation: {:.4}%", r_variation * 100.0);
    println!("  Energy error: {:.2e}", energy_error);

    assert!(r_variation < 0.001, "Circular orbit radius varied too much!");
    assert!(energy_error < 1e-6, "Energy not conserved!");

    println!("  PASSED\n");
}

fn test_elliptical_orbit(perihelion_au: f64, eccentricity: f64) {
    println!("Testing elliptical orbit (e={:.1}) with perihelion at {:.2} AU...", eccentricity, perihelion_au);

    let r_p = perihelion_au * AU_TO_METERS;
    let a = r_p / (1.0 - eccentricity); // Semi-major axis
    let r_a = a * (1.0 + eccentricity); // Aphelion

    let mut pos = DVec2::new(r_p, 0.0);

    // Velocity at perihelion: v² = GM * (2/r - 1/a)
    let v_perihelion = ((SUN_GM) * (2.0 / r_p - 1.0 / a)).sqrt();
    let mut vel = DVec2::new(0.0, v_perihelion);

    let mut acc = compute_acceleration_sun_only(pos);

    // Expected orbital period
    let period = 2.0 * std::f64::consts::PI * (a.powi(3) / SUN_GM).sqrt();
    let period_days = period / SECONDS_PER_DAY;

    println!("  Perihelion: {:.4} AU, Aphelion: {:.4} AU", r_p / AU_TO_METERS, r_a / AU_TO_METERS);
    println!("  Semi-major axis: {:.4} AU", a / AU_TO_METERS);
    println!("  Expected period: {:.2} days", period_days);

    // Use smaller timestep for elliptical orbit
    let dt = 600.0; // 10 minutes
    let steps_per_day = (SECONDS_PER_DAY / dt) as usize;
    let total_steps = (period / dt) as usize + steps_per_day;

    // Initial energy
    let e0 = 0.5 * vel.length_squared() - SUN_GM / pos.length();

    let mut t = 0.0;
    let mut min_r = r_p;
    let mut max_r = r_p;

    for step in 0..total_steps {
        verlet_step(&mut pos, &mut vel, &mut acc, dt);
        t += dt;

        let r = pos.length();
        min_r = min_r.min(r);
        max_r = max_r.max(r);

        // Print progress every 30 days
        if step % (30 * steps_per_day) == 0 && step > 0 {
            println!("    Day {:>6.0}: r = {:.4} AU", t / SECONDS_PER_DAY, r / AU_TO_METERS);
        }
    }

    // Final energy
    let ef = 0.5 * vel.length_squared() - SUN_GM / pos.length();
    let energy_error = (ef - e0).abs() / e0.abs();

    // Check orbital bounds
    let perihelion_error = (min_r - r_p).abs() / r_p;
    let aphelion_error = (max_r - r_a).abs() / r_a;

    println!("  Measured perihelion: {:.4} AU (error: {:.3}%)", min_r / AU_TO_METERS, perihelion_error * 100.0);
    println!("  Measured aphelion: {:.4} AU (error: {:.3}%)", max_r / AU_TO_METERS, aphelion_error * 100.0);
    println!("  Energy error: {:.2e}", energy_error);

    assert!(perihelion_error < 0.01, "Perihelion error too large!");
    assert!(aphelion_error < 0.01, "Aphelion error too large!");
    assert!(energy_error < 1e-5, "Energy not conserved!");

    println!("  PASSED\n");
}
