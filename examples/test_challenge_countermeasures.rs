//! Test each countermeasure type in the Challenge scenario.
//!
//! Simulates deflection of a 1×10⁹ kg asteroid on collision course with Earth.
//! Tests: DART-style kinetic, heavy kinetic, nuclear standoff, nuclear split.

use bevy::math::DVec2;
use deorbiting::ephemeris::{COLLISION_MULTIPLIER, CelestialBodyId, Ephemeris};
use deorbiting::interceptor::DeflectionPayload;
use deorbiting::physics::compute_acceleration;
use deorbiting::types::AU_TO_METERS;

/// Challenge scenario parameters
const ASTEROID_MASS: f64 = 1e9; // 1 billion kg (~80m diameter)

/// Propagate asteroid and check for Earth collision.
/// Returns (collides_with_earth, closest_approach_to_earth_in_km)
fn simulate_trajectory(
    mut pos: DVec2,
    mut vel: DVec2,
    start_time: f64,
    duration: f64,
    ephemeris: &Ephemeris,
) -> (bool, f64) {
    let dt = 3600.0; // 1 hour timestep
    let mut t = start_time;
    let end_time = start_time + duration;
    let mut closest_approach = f64::MAX;

    // Get Earth collision radius
    let earth_data = ephemeris
        .get_body_data_by_id(CelestialBodyId::Earth)
        .unwrap();
    let collision_radius = earth_data.radius * COLLISION_MULTIPLIER;

    while t < end_time {
        let step = (end_time - t).min(dt);

        // Velocity Verlet integration
        let acc1 = compute_acceleration(pos, t, ephemeris);
        let half_vel = vel + acc1 * step * 0.5;
        pos = pos + half_vel * step;
        let acc2 = compute_acceleration(pos, t + step, ephemeris);
        vel = half_vel + acc2 * step * 0.5;
        t += step;

        // Check distance to Earth
        let earth_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, t)
            .unwrap();
        let distance = (pos - earth_pos).length();
        closest_approach = closest_approach.min(distance);

        // Check for collision
        if distance < collision_radius {
            return (true, distance / 1000.0);
        }
    }

    (false, closest_approach / 1000.0)
}

/// Propagate asteroid forward for a given time, returning new state.
fn propagate_asteroid(
    mut pos: DVec2,
    mut vel: DVec2,
    start_time: f64,
    duration: f64,
    ephemeris: &Ephemeris,
) -> (DVec2, DVec2, f64) {
    let dt = 3600.0;
    let mut t = start_time;
    let end_time = start_time + duration;

    while t < end_time {
        let step = (end_time - t).min(dt);
        let acc1 = compute_acceleration(pos, t, ephemeris);
        let half_vel = vel + acc1 * step * 0.5;
        pos = pos + half_vel * step;
        let acc2 = compute_acceleration(pos, t + step, ephemeris);
        vel = half_vel + acc2 * step * 0.5;
        t += step;
    }

    (pos, vel, t)
}

/// Set up Challenge scenario initial conditions.
/// Asteroid 91° ahead of Earth, moving retrograde.
fn setup_challenge_scenario(ephemeris: &Ephemeris, start_time: f64) -> (DVec2, DVec2) {
    // 91° ahead takes ~92 days at Earth's angular rate
    let days_for_91_degrees = 91.0 / 0.9856;
    let time_offset = days_for_91_degrees * 86400.0;
    let future_time = start_time + time_offset;

    // Get Earth's position 91° ahead
    let pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, future_time)
        .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

    // Compute Earth's velocity via numerical differentiation
    let dt = 60.0;
    let pos_before = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, future_time - dt)
        .unwrap_or(pos);
    let pos_after = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, future_time + dt)
        .unwrap_or(pos);
    let earth_velocity = (pos_after - pos_before) / (2.0 * dt);

    // Retrograde: asteroid travels opposite to Earth
    let vel = -earth_velocity;

    (pos, vel)
}

/// Calculate deflection based on payload and direction.
fn apply_deflection_with_direction(
    vel: DVec2,
    payload: &DeflectionPayload,
    asteroid_mass: f64,
    direction: DVec2,
) -> DVec2 {
    let relative_velocity = 8_000.0; // 8 km/s typical
    let delta_v = payload.calculate_delta_v(
        asteroid_mass,
        relative_velocity,
        direction.normalize_or_zero(),
    );
    vel + delta_v
}

/// Get standard deflection directions for testing.
fn get_deflection_directions(vel: DVec2) -> Vec<(&'static str, DVec2)> {
    let retrograde = -vel.normalize_or_zero();
    let prograde = vel.normalize_or_zero();
    // Perpendicular: rotate 90° counterclockwise
    let perp_left = DVec2::new(-vel.y, vel.x).normalize_or_zero();
    // Perpendicular: rotate 90° clockwise
    let perp_right = DVec2::new(vel.y, -vel.x).normalize_or_zero();

    vec![
        ("retrograde", retrograde),
        ("prograde", prograde),
        ("perpendicular (left)", perp_left),
        ("perpendicular (right)", perp_right),
    ]
}

/// Simulate trajectory without early collision termination.
/// Returns closest approach distance in km.
fn simulate_closest_approach(
    mut pos: DVec2,
    mut vel: DVec2,
    start_time: f64,
    duration: f64,
    ephemeris: &Ephemeris,
) -> f64 {
    let dt = 3600.0;
    let mut t = start_time;
    let end_time = start_time + duration;
    let mut closest_approach = f64::MAX;

    while t < end_time {
        let step = (end_time - t).min(dt);
        let acc1 = compute_acceleration(pos, t, ephemeris);
        let half_vel = vel + acc1 * step * 0.5;
        pos = pos + half_vel * step;
        let acc2 = compute_acceleration(pos, t + step, ephemeris);
        vel = half_vel + acc2 * step * 0.5;
        t += step;

        let earth_pos = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, t)
            .unwrap();
        let distance = (pos - earth_pos).length();
        closest_approach = closest_approach.min(distance);
    }

    closest_approach / 1000.0
}

fn main() {
    println!("=== Challenge Scenario Countermeasure Test ===\n");
    println!("Asteroid mass: {:.2e} kg (~80m diameter)", ASTEROID_MASS);
    println!("Scenario: ~46 day warning, asteroid 91° ahead of Earth, retrograde");

    let ephemeris = Ephemeris::default();
    let start_time = 0.0; // J2000

    // Set up initial conditions
    let (asteroid_pos, asteroid_vel) = setup_challenge_scenario(&ephemeris, start_time);

    println!(
        "Initial position: {:.4} AU from Sun",
        asteroid_pos.length() / AU_TO_METERS
    );
    println!(
        "Initial velocity: {:.2} km/s (retrograde)",
        asteroid_vel.length() / 1000.0
    );

    // Show collision radii for reference
    let earth_data = ephemeris
        .get_body_data_by_id(CelestialBodyId::Earth)
        .unwrap();
    let real_earth_radius_km = earth_data.radius / 1000.0;
    let gameplay_collision_km = real_earth_radius_km * COLLISION_MULTIPLIER;
    println!("Earth radius: {:.0} km", real_earth_radius_km);
    println!(
        "Gameplay collision radius: {:.0} km ({}× Earth radius)\n",
        gameplay_collision_km, COLLISION_MULTIPLIER
    );

    let simulation_days = 60.0;

    // Baseline
    println!("--- Baseline (no deflection) ---");
    let baseline_closest = simulate_closest_approach(
        asteroid_pos,
        asteroid_vel,
        start_time,
        simulation_days * 86400.0,
        &ephemeris,
    );
    let baseline_gameplay = if baseline_closest < gameplay_collision_km {
        "COLLISION"
    } else {
        "MISS"
    };
    let baseline_real = if baseline_closest < real_earth_radius_km {
        "COLLISION"
    } else {
        "MISS"
    };
    println!(
        "Closest approach: {:.0} km ({:.1} Earth radii)",
        baseline_closest,
        baseline_closest / real_earth_radius_km
    );
    println!("  Gameplay rules: {}", baseline_gameplay);
    println!("  Real physics:   {}", baseline_real);
    println!();

    // Define countermeasures to test
    let countermeasures: Vec<(&str, DeflectionPayload)> = vec![
        ("DART-style Kinetic (50t)", DeflectionPayload::dart()),
        ("Heavy Kinetic (250t)", DeflectionPayload::heavy_kinetic()),
        ("Nuclear (100 kt)", DeflectionPayload::nuclear(100.0)),
        ("Nuclear (500 kt)", DeflectionPayload::nuclear(500.0)),
        ("Nuclear (1000 kt)", DeflectionPayload::nuclear(1000.0)),
    ];

    // Get deflection directions
    let directions = get_deflection_directions(asteroid_vel);

    // First, find the best direction for each countermeasure
    println!("--- Testing Deflection Directions (immediate, ~46 day warning) ---\n");

    let mut best_results: Vec<(&str, &str, f64, f64)> = Vec::new(); // (payload, direction, closest, improvement)

    for (payload_name, payload) in &countermeasures {
        println!("{}:", payload_name);
        let mut best_closest = 0.0;
        let mut best_dir = "";

        for (dir_name, direction) in &directions {
            let deflected_vel =
                apply_deflection_with_direction(asteroid_vel, payload, ASTEROID_MASS, *direction);
            let dv = (deflected_vel - asteroid_vel).length();

            let closest = simulate_closest_approach(
                asteroid_pos,
                deflected_vel,
                start_time,
                simulation_days * 86400.0,
                &ephemeris,
            );

            let change = closest - baseline_closest;
            let sign = if change >= 0.0 { "+" } else { "" };

            let status = if closest >= gameplay_collision_km {
                "✓ GAMEPLAY MISS"
            } else if closest >= real_earth_radius_km {
                "real miss"
            } else {
                "collision"
            };

            println!(
                "  {}: Δv={:.3} m/s → {:.0} km ({}{:.0}) {}",
                dir_name, dv, closest, sign, change, status
            );

            if closest > best_closest {
                best_closest = closest;
                best_dir = dir_name;
            }
        }

        best_results.push((
            payload_name,
            best_dir,
            best_closest,
            best_closest - baseline_closest,
        ));
        println!();
    }

    // Summary
    println!("=== Summary ===\n");
    println!(
        "Baseline: {:.0} km ({:.1} Earth radii) - {} in gameplay, {} in reality",
        baseline_closest,
        baseline_closest / real_earth_radius_km,
        baseline_gameplay,
        baseline_real
    );
    println!(
        "Gameplay collision threshold: {:.0} km\n",
        gameplay_collision_km
    );

    println!("Best direction for each countermeasure:");
    let mut gameplay_wins = Vec::new();
    for (payload, best_dir, closest, improvement) in &best_results {
        let status = if *closest >= gameplay_collision_km {
            gameplay_wins.push((*payload, *best_dir));
            "✓ DEFLECTED"
        } else {
            "still collides"
        };
        println!(
            "  {} + {} → {:.0} km (+{:.0} km) {}",
            payload, best_dir, closest, improvement, status
        );
    }

    println!("\n--- Countermeasures that deflect asteroid (gameplay rules) ---");
    if gameplay_wins.is_empty() {
        println!("  NONE - asteroid mass too large for available countermeasures");
        println!(
            "  Need to push asteroid {:.0} km further to avoid gameplay collision",
            gameplay_collision_km - baseline_closest
        );
    } else {
        for (payload, dir) in &gameplay_wins {
            println!("  ✓ {} ({})", payload, dir);
        }
    }
}
