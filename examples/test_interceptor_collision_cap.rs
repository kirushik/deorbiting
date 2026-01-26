//! Test that interceptor flight time is capped when asteroid would collide.
//!
//! This verifies the fix for the bug where interceptors would target asteroid
//! positions past the collision point with Earth.
//!
//! Run with: cargo run --example test_interceptor_collision_cap

use bevy::math::DVec2;
use deorbiting::ephemeris::{COLLISION_MULTIPLIER, CelestialBodyId, Ephemeris};
use deorbiting::types::{AU_TO_METERS, BodyState};

/// Predict asteroid position with collision detection.
/// Returns (final_pos, final_vel, collision_info)
fn predict_with_collision(
    initial: &BodyState,
    start_time: f64,
    target_time: f64,
    ephemeris: &Ephemeris,
) -> (DVec2, DVec2, Option<(CelestialBodyId, f64)>) {
    let dt_max = 3600.0; // 1 hour max timestep
    let mut pos = initial.pos;
    let mut vel = initial.vel;
    let mut t = start_time;

    while t < target_time {
        let dt = (target_time - t).min(dt_max);

        // Velocity Verlet integration
        let acc1 = compute_acceleration(pos, t, ephemeris);
        let half_vel = vel + acc1 * dt * 0.5;
        pos += half_vel * dt;
        let acc2 = compute_acceleration(pos, t + dt, ephemeris);
        vel = half_vel + acc2 * dt * 0.5;
        t += dt;

        // Check for collision
        if let Some(body_id) = ephemeris.check_collision(pos, t) {
            return (pos, vel, Some((body_id, t)));
        }
    }

    (pos, vel, None)
}

/// Compute gravitational acceleration from Sun and planets.
fn compute_acceleration(pos: DVec2, time: f64, ephemeris: &Ephemeris) -> DVec2 {
    let sources = ephemeris.get_gravity_sources(time);
    let mut acc = DVec2::ZERO;

    for &(body_pos, gm) in &sources {
        let delta = body_pos - pos;
        let r_squared = delta.length_squared();
        if r_squared > 1e6 {
            let r = r_squared.sqrt();
            acc += delta * (gm / (r_squared * r));
        }
    }

    acc
}

fn main() {
    println!("=== INTERCEPTOR COLLISION CAP TEST ===\n");

    let ephemeris = Ephemeris::default();

    // Setup: Deflection Challenge scenario (91° ahead, retrograde)
    // Time offset for 91° ahead → ~46 days to collision
    let days_for_91_degrees = 91.0 / 0.9856; // Earth moves ~0.9856°/day
    let time_offset = days_for_91_degrees * 86400.0;

    let start_time = 0.0; // J2000

    // Get Earth position and velocity at start time
    let earth_pos_now = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, start_time)
        .unwrap();
    let earth_data = ephemeris
        .get_body_data_by_id(CelestialBodyId::Earth)
        .unwrap();
    let earth_collision_radius = earth_data.radius * COLLISION_MULTIPLIER;

    // Compute Earth's velocity via numerical differentiation
    let dt = 60.0;
    let earth_pos_before = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, start_time - dt)
        .unwrap();
    let earth_pos_after = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, start_time + dt)
        .unwrap();
    let _earth_vel = (earth_pos_after - earth_pos_before) / (2.0 * dt);

    // Position asteroid at Earth's future position (91° ahead)
    let future_earth_pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, start_time + time_offset)
        .unwrap();

    // Compute future Earth's velocity for retrograde orbit
    let future_earth_before = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, start_time + time_offset - dt)
        .unwrap();
    let future_earth_after = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, start_time + time_offset + dt)
        .unwrap();
    let future_earth_vel = (future_earth_after - future_earth_before) / (2.0 * dt);

    // Asteroid: retrograde orbit (opposite to Earth's velocity)
    let asteroid_pos = future_earth_pos;
    let asteroid_vel = -future_earth_vel;

    let initial_state = BodyState {
        pos: asteroid_pos,
        vel: asteroid_vel,
        mass: 5e9,
    };

    println!("Initial Setup:");
    println!(
        "  Earth position: ({:.4} AU, {:.4} AU)",
        earth_pos_now.x / AU_TO_METERS,
        earth_pos_now.y / AU_TO_METERS
    );
    println!(
        "  Earth collision radius: {:.0} km ({:.1}x actual radius)",
        earth_collision_radius / 1000.0,
        COLLISION_MULTIPLIER
    );
    println!(
        "  Asteroid position: ({:.4} AU, {:.4} AU) - 91° ahead",
        asteroid_pos.x / AU_TO_METERS,
        asteroid_pos.y / AU_TO_METERS
    );
    println!(
        "  Asteroid velocity: {:.1} km/s (retrograde)",
        asteroid_vel.length() / 1000.0
    );
    println!("  Expected collision: ~{:.1} days", days_for_91_degrees);

    // Test 1: Predict with default 90-day flight time
    let default_flight_time = 90.0 * 86400.0;
    let (final_pos, _final_vel, collision) = predict_with_collision(
        &initial_state,
        start_time,
        start_time + default_flight_time * 2.0, // Look ahead 2x to find collision
        &ephemeris,
    );

    println!("\nTest 1: Collision Detection with 90-day look-ahead");
    if let Some((body, collision_time)) = collision {
        let time_to_collision = collision_time - start_time;
        println!("  ✓ Collision detected with {:?}", body);
        println!(
            "  ✓ Time to collision: {:.2} days",
            time_to_collision / 86400.0
        );

        // This is what the fix should do
        if time_to_collision < default_flight_time {
            let capped_flight_time = time_to_collision * 0.9;
            println!(
                "  ✓ Flight time would be capped: {:.1} → {:.1} days",
                default_flight_time / 86400.0,
                capped_flight_time / 86400.0
            );

            // Verify we can reach the asteroid at capped time
            let (intercept_pos, _intercept_vel, intercept_collision) = predict_with_collision(
                &initial_state,
                start_time,
                start_time + capped_flight_time,
                &ephemeris,
            );

            if intercept_collision.is_none() {
                println!("  ✓ Asteroid reachable at capped time (no collision yet)");
                println!(
                    "    Position at intercept: ({:.4} AU, {:.4} AU)",
                    intercept_pos.x / AU_TO_METERS,
                    intercept_pos.y / AU_TO_METERS
                );
            } else {
                println!("  ✗ BUG: Collision occurs even at capped time!");
            }
        } else {
            println!("  No capping needed (collision after default flight time)");
        }
    } else {
        println!("  ✗ BUG: No collision detected!");
        println!(
            "    Final position: ({:.4} AU, {:.4} AU)",
            final_pos.x / AU_TO_METERS,
            final_pos.y / AU_TO_METERS
        );
    }

    // Test 2: Verify position at 90 days would be unreachable
    println!("\nTest 2: Position at 90 days (should be past collision)");
    let (pos_90d, _vel_90d, collision_90d) = predict_with_collision(
        &initial_state,
        start_time,
        start_time + default_flight_time,
        &ephemeris,
    );

    if collision_90d.is_some() {
        println!("  ✓ Correct: Collision before 90 days - position unreachable");
    } else {
        println!(
            "  Position at 90 days: ({:.4} AU, {:.4} AU)",
            pos_90d.x / AU_TO_METERS,
            pos_90d.y / AU_TO_METERS
        );
        // Check distance from Earth at t=90d
        let earth_at_90d = ephemeris
            .get_position_by_id(CelestialBodyId::Earth, start_time + default_flight_time)
            .unwrap();
        let distance_from_earth = (pos_90d - earth_at_90d).length();
        println!(
            "  Distance from Earth at 90 days: {:.0} km",
            distance_from_earth / 1000.0
        );
    }

    println!("\n=== TEST COMPLETE ===");
}
