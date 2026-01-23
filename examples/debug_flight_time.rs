//! Debug example to investigate flight time calculations.
//!
//! Run with: cargo run --example debug_flight_time

use bevy::math::DVec2;

use deorbiting::ui::deflection_helpers::{
    BASE_INTERCEPTOR_SPEED, calculate_flight_time_from_earth,
};

const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;

fn main() {
    println!("=== Flight Time Debugging ===\n");
    println!(
        "BASE_INTERCEPTOR_SPEED = {} m/s ({} km/s)",
        BASE_INTERCEPTOR_SPEED,
        BASE_INTERCEPTOR_SPEED / 1000.0
    );
    println!("1 AU = {} meters\n", AU_TO_METERS);

    // Earth at 1 AU on x-axis
    let earth_pos = DVec2::new(AU_TO_METERS, 0.0);

    // Test various asteroid distances
    let test_cases = [
        (
            "Asteroid 0.1 AU from Earth",
            DVec2::new(AU_TO_METERS + 0.1 * AU_TO_METERS, 0.0),
        ),
        (
            "Asteroid 0.5 AU from Earth",
            DVec2::new(AU_TO_METERS + 0.5 * AU_TO_METERS, 0.0),
        ),
        (
            "Asteroid 1 AU from Earth (opposite side)",
            DVec2::new(-AU_TO_METERS, 0.0),
        ),
        ("Asteroid 90° from Earth", DVec2::new(0.0, AU_TO_METERS)),
        (
            "Near-Earth (0.01 AU)",
            DVec2::new(AU_TO_METERS + 0.01 * AU_TO_METERS, 0.0),
        ),
        (
            "Very near (0.001 AU = ~150,000 km)",
            DVec2::new(AU_TO_METERS + 0.001 * AU_TO_METERS, 0.0),
        ),
    ];

    for (name, asteroid_pos) in test_cases {
        let distance = (asteroid_pos - earth_pos).length();
        let flight_time_seconds = calculate_flight_time_from_earth(asteroid_pos, earth_pos);
        let flight_time_days = flight_time_seconds / SECONDS_PER_DAY;

        println!("{}", name);
        println!(
            "  Distance: {:.2e} m ({:.4} AU, {:.0} km)",
            distance,
            distance / AU_TO_METERS,
            distance / 1000.0
        );
        println!(
            "  Flight time: {:.1} days ({:.2e} seconds)\n",
            flight_time_days, flight_time_seconds
        );
    }

    // What speed would be needed for reasonable flight times?
    println!("=== Alternative speeds analysis ===\n");

    let target_distance = 0.5 * AU_TO_METERS; // Half AU
    for &target_days in &[7.0, 14.0, 30.0, 60.0, 90.0] {
        let needed_speed = target_distance / (target_days * SECONDS_PER_DAY);
        println!(
            "For 0.5 AU in {} days: need {} km/s",
            target_days,
            needed_speed / 1000.0
        );
    }

    println!("\n=== Current speeds produce these flight times for 0.5 AU ===\n");
    for &speed in &[15_000.0, 30_000.0, 50_000.0, 100_000.0, 150_000.0] {
        let flight_days = (target_distance / speed) / SECONDS_PER_DAY;
        println!("  {} km/s -> {:.1} days", speed / 1000.0, flight_days);
    }
}
