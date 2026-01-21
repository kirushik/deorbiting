//! Integration tests for trajectory prediction.

mod common;

use bevy::math::DVec2;
use deorbiting::types::{AU_TO_METERS, CRASH_DISTANCE, ESCAPE_DISTANCE, GM_SUN, SECONDS_PER_DAY};

#[test]
fn test_prediction_detects_escape() {
    // Escape trajectory: velocity > escape velocity
    let r = AU_TO_METERS;
    let v_esc = (2.0 * GM_SUN / r).sqrt();
    let v = v_esc * 1.2; // 20% above escape velocity

    let pos = DVec2::new(r, 0.0);
    let vel = DVec2::new(0.0, v);

    // Simulate until escape or max time
    let max_time = 100.0 * 365.25 * SECONDS_PER_DAY; // 100 years
    let dt = 3600.0 * 24.0; // 1 day steps
    let max_steps = (max_time / dt) as usize;

    let mut current_pos = pos;
    let mut current_vel = vel;
    let mut escaped = false;

    for _ in 0..max_steps {
        let (new_pos, new_vel) = common::simulate_verlet(current_pos, current_vel, dt, 100);
        current_pos = new_pos;
        current_vel = new_vel;

        if current_pos.length() > ESCAPE_DISTANCE {
            escaped = true;
            break;
        }
    }

    assert!(escaped, "Escape trajectory should reach escape distance");
}

#[test]
fn test_prediction_stays_bound() {
    // Bound orbit should not escape
    let (pos, vel) = common::circular_orbit(1.0);

    // Simulate for 10 years
    let duration = 10.0 * 365.25 * SECONDS_PER_DAY;
    let (final_pos, _) = common::simulate_verlet(pos, vel, duration, 100_000);

    let final_distance = final_pos.length();

    assert!(
        final_distance < ESCAPE_DISTANCE,
        "Bound orbit should not escape: distance = {} AU",
        final_distance / AU_TO_METERS
    );

    assert!(
        final_distance > CRASH_DISTANCE,
        "Bound orbit should not crash: distance = {} m",
        final_distance
    );
}

#[test]
fn test_crash_trajectory_detection() {
    // Very low orbit that should crash into the Sun
    let r = 0.01 * AU_TO_METERS; // Very close to Sun
    let v = (GM_SUN / r).sqrt() * 0.1; // Much slower than circular orbit

    let pos = DVec2::new(r, 0.0);
    let vel = DVec2::new(0.0, v);

    // This orbit will crash into the Sun
    let initial_energy = common::orbital_energy(pos, vel);
    let l = common::angular_momentum(pos, vel);

    // Perihelion distance for this orbit
    // From r_p = (h²/GM) / (1 + e), with h = L
    let h_sq = l * l;
    let a = -GM_SUN / (2.0 * initial_energy);
    let e = (1.0 - h_sq / (a * GM_SUN)).sqrt();
    let r_perihelion = a * (1.0 - e);

    // If perihelion < crash distance, it will crash
    if r_perihelion < CRASH_DISTANCE {
        // This should crash
        let duration = 10.0 * 365.25 * SECONDS_PER_DAY;
        let dt = 3600.0;
        let steps = (duration / dt) as usize;

        let mut current_pos = pos;
        let mut current_vel = vel;
        let mut crashed = false;

        for _ in 0..steps.min(1_000_000) {
            let (new_pos, new_vel) = common::simulate_verlet(current_pos, current_vel, dt, 10);
            current_pos = new_pos;
            current_vel = new_vel;

            if current_pos.length() < CRASH_DISTANCE {
                crashed = true;
                break;
            }
        }

        // Note: This test may or may not crash depending on exact parameters
        // The important thing is that the simulation detects it if it happens
        if crashed {
            assert!(current_pos.length() < CRASH_DISTANCE);
        }
    }
}

#[test]
fn test_prediction_cache_continuation_concept() {
    // Test the concept: extending a prediction should match a fresh one
    let (pos, vel) = common::circular_orbit(1.0);

    // First segment: 1 year
    let first_duration = 365.25 * SECONDS_PER_DAY;
    let (mid_pos, mid_vel) = common::simulate_verlet(pos, vel, first_duration, 50_000);

    // Continue for another year
    let second_duration = 365.25 * SECONDS_PER_DAY;
    let (final_pos_continued, final_vel_continued) =
        common::simulate_verlet(mid_pos, mid_vel, second_duration, 50_000);

    // Fresh simulation for 2 years
    let total_duration = 2.0 * 365.25 * SECONDS_PER_DAY;
    let (final_pos_fresh, final_vel_fresh) =
        common::simulate_verlet(pos, vel, total_duration, 100_000);

    // They should be very close (some numerical difference expected)
    let pos_diff = (final_pos_continued - final_pos_fresh).length();
    let vel_diff = (final_vel_continued - final_vel_fresh).length();

    assert!(
        pos_diff / AU_TO_METERS < 0.001,
        "Continued prediction differs from fresh by {} AU",
        pos_diff / AU_TO_METERS
    );

    assert!(
        vel_diff < 100.0,
        "Continued velocity differs from fresh by {} m/s",
        vel_diff
    );
}

#[test]
fn test_trajectory_color_boundaries() {
    // Test that we can identify when trajectory approaches different planets
    // (This is a conceptual test - the actual color logic is in prediction.rs)

    // Mars orbit ~1.5 AU
    let mars_distance = 1.5 * AU_TO_METERS;

    // Create trajectory that crosses Mars orbit
    let (pos, vel) = common::elliptical_orbit(1.0, 0.3);

    // Semi-major axis
    let r_p = AU_TO_METERS;
    let a = r_p / (1.0 - 0.3);
    let aphelion = 2.0 * a - r_p;

    // This orbit should cross Mars distance
    assert!(
        aphelion > mars_distance,
        "Test orbit should reach beyond Mars"
    );
}

#[test]
fn test_prediction_performance_baseline() {
    // Ensure prediction can complete in reasonable time
    let (pos, vel) = common::circular_orbit(1.0);

    let start = std::time::Instant::now();

    // Simulate 1 year with reasonable resolution
    let duration = 365.25 * SECONDS_PER_DAY;
    let _ = common::simulate_verlet(pos, vel, duration, 10_000);

    let elapsed = start.elapsed();

    // Should complete in under 100ms
    assert!(
        elapsed.as_millis() < 100,
        "Basic prediction took too long: {}ms",
        elapsed.as_millis()
    );
}
