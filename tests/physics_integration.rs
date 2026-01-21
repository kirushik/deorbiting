//! Integration tests for physics simulation.

mod common;

use approx::assert_relative_eq;
use bevy::math::DVec2;
use deorbiting::types::{AU_TO_METERS, GM_SUN, SECONDS_PER_DAY};

#[test]
fn test_earth_orbit_365_days() {
    // Earth-like circular orbit at 1 AU
    let (pos, vel) = common::circular_orbit(1.0);

    // Expected orbital period: ~365.25 days
    let a = AU_TO_METERS;
    let expected_period = common::orbital_period(a);
    let expected_days = expected_period / SECONDS_PER_DAY;

    // Verify it's close to 365.25 days
    assert_relative_eq!(expected_days, 365.25, epsilon = 0.5);

    // Simulate one full orbit
    let (final_pos, _final_vel) = common::simulate_verlet(
        pos,
        vel,
        expected_period,
        100_000, // High resolution for accuracy
    );

    // Should return close to starting position
    let distance = (final_pos - pos).length();
    let distance_au = distance / AU_TO_METERS;

    assert!(
        distance_au < 0.001,
        "Earth should return to within 0.001 AU after one orbit, got {} AU",
        distance_au
    );
}

#[test]
fn test_retrograde_orbit_stable() {
    // Retrograde orbit: velocity in opposite direction
    let r = AU_TO_METERS;
    let v = (GM_SUN / r).sqrt();
    let pos = DVec2::new(r, 0.0);
    let vel = DVec2::new(0.0, -v); // Retrograde (clockwise)

    let initial_energy = common::orbital_energy(pos, vel);

    // Simulate for one year
    let duration = 365.25 * SECONDS_PER_DAY;
    let (final_pos, final_vel) = common::simulate_verlet(pos, vel, duration, 100_000);

    let final_energy = common::orbital_energy(final_pos, final_vel);

    // Energy should be conserved
    let drift = ((final_energy - initial_energy) / initial_energy).abs();
    assert!(
        drift < 0.01,
        "Retrograde orbit energy drift {} exceeds 1%",
        drift
    );
}

#[test]
fn test_highly_elliptical_orbit() {
    // Highly elliptical orbit (e = 0.95)
    let (pos, vel) = common::elliptical_orbit(0.5, 0.95);

    let initial_energy = common::orbital_energy(pos, vel);
    let initial_l = common::angular_momentum(pos, vel);

    // Semi-major axis
    let r_p = 0.5 * AU_TO_METERS;
    let a = r_p / (1.0 - 0.95);
    let period = common::orbital_period(a);

    // Simulate one orbit (high eccentricity needs more steps)
    let (final_pos, final_vel) = common::simulate_verlet(pos, vel, period, 500_000);

    let final_energy = common::orbital_energy(final_pos, final_vel);
    let final_l = common::angular_momentum(final_pos, final_vel);

    // Energy conservation (allow more drift for extreme orbits)
    let energy_drift = ((final_energy - initial_energy) / initial_energy).abs();
    assert!(
        energy_drift < 0.05,
        "High eccentricity orbit energy drift {} exceeds 5%",
        energy_drift
    );

    // Angular momentum conservation
    let l_drift = ((final_l - initial_l) / initial_l).abs();
    assert!(
        l_drift < 0.01,
        "High eccentricity orbit L drift {} exceeds 1%",
        l_drift
    );
}

#[test]
fn test_long_term_stability_10_years() {
    // Test stability over 10 orbits (Earth-like)
    let (pos, vel) = common::circular_orbit(1.0);
    let initial_energy = common::orbital_energy(pos, vel);

    // 10 years
    let duration = 10.0 * 365.25 * SECONDS_PER_DAY;
    let (final_pos, final_vel) = common::simulate_verlet(pos, vel, duration, 1_000_000);

    let final_energy = common::orbital_energy(final_pos, final_vel);

    // Energy should drift less than 1% over 10 years
    let drift = ((final_energy - initial_energy) / initial_energy).abs();
    assert!(drift < 0.01, "10-year energy drift {} exceeds 1%", drift);
}

#[test]
fn test_escape_velocity_boundary() {
    // Just below escape velocity - should be bound
    let r = AU_TO_METERS;
    let v_esc = (2.0 * GM_SUN / r).sqrt();
    let v_below = v_esc * 0.95;

    let pos = DVec2::new(r, 0.0);
    let vel_below = DVec2::new(0.0, v_below);

    let energy_below = common::orbital_energy(pos, vel_below);
    assert!(energy_below < 0.0, "95% of escape velocity should be bound");

    // Just above escape velocity - should escape
    let v_above = v_esc * 1.05;
    let vel_above = DVec2::new(0.0, v_above);

    let energy_above = common::orbital_energy(pos, vel_above);
    assert!(energy_above > 0.0, "105% of escape velocity should escape");
}

#[test]
fn test_vis_viva_equation() {
    // Vis-viva equation: v² = GM * (2/r - 1/a)
    let a = 2.0 * AU_TO_METERS; // 2 AU semi-major axis
    let r_p = 1.0 * AU_TO_METERS; // 1 AU perihelion
    let e = 1.0 - r_p / a; // eccentricity

    let (pos, vel) = common::elliptical_orbit(1.0, e);

    // At perihelion, vis-viva gives:
    let v_expected = (GM_SUN * (2.0 / r_p - 1.0 / a)).sqrt();
    let v_actual = vel.length();

    assert_relative_eq!(v_actual, v_expected, epsilon = 1.0);
}

#[test]
fn test_angular_momentum_at_perihelion_aphelion() {
    // Angular momentum L = r × v is constant
    // At perihelion: L = r_p * v_p
    // At aphelion: L = r_a * v_a
    // Therefore: r_p * v_p = r_a * v_a

    let (pos, vel) = common::elliptical_orbit(1.0, 0.5);
    let initial_l = common::angular_momentum(pos, vel);

    // Semi-major axis and aphelion
    let r_p = AU_TO_METERS;
    let a = r_p / (1.0 - 0.5);
    let r_a = 2.0 * a - r_p;

    // Simulate to aphelion (half orbit)
    let period = common::orbital_period(a);
    let (final_pos, final_vel) = common::simulate_verlet(pos, vel, period / 2.0, 100_000);

    let final_l = common::angular_momentum(final_pos, final_vel);

    // L should be the same at aphelion
    let l_diff = ((final_l - initial_l) / initial_l).abs();
    assert!(
        l_diff < 0.001,
        "Angular momentum changed by {} at aphelion",
        l_diff
    );

    // Verify we're actually at aphelion (roughly)
    let final_r = final_pos.length();
    let r_error = (final_r - r_a).abs() / r_a;
    assert!(
        r_error < 0.01,
        "Expected aphelion distance {} AU, got {} AU",
        r_a / AU_TO_METERS,
        final_r / AU_TO_METERS
    );
}

#[test]
fn test_mercury_orbit_period() {
    // Mercury: a ≈ 0.387 AU, period ≈ 88 days
    let a = 0.387 * AU_TO_METERS;
    let period = common::orbital_period(a);
    let period_days = period / SECONDS_PER_DAY;

    assert_relative_eq!(period_days, 88.0, epsilon = 1.0);
}
