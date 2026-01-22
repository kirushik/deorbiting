//! Diagnostic test for long extrapolation beyond ephemeris table coverage.
//!
//! This example reproduces the issue where planet orbits become erratic
//! when simulation time goes centuries beyond table coverage (e.g., year 2844).
//!
//! Run with: cargo run --example test_long_extrapolation
//!
//! Expected behavior BEFORE fix:
//! - Orbits at year 2844 are offset from Sun (constant dp offset accumulates error)
//! - Orbit center is significantly displaced from origin
//!
//! Expected behavior AFTER fix:
//! - Orbits at year 2844 are clean ellipses centered on Sun
//! - Pure Kepler fallback produces stable orbits

use deorbiting::ephemeris::{CelestialBodyId, Ephemeris};
use deorbiting::types::{AU_TO_METERS, SECONDS_PER_DAY};

fn main() {
    println!("=== Long Extrapolation Diagnostic ===\n");

    let eph = Ephemeris::new();

    // Check if we have Horizons tables loaded
    let has_tables = eph.horizons_coverage(CelestialBodyId::Earth).is_some();
    if !has_tables {
        println!("Note: No Horizons tables loaded, using pure Kepler throughout.");
        println!("This test is most meaningful with tables present.\n");
    }

    // Get table coverage info
    if let Some(cov) = eph.horizons_coverage(CelestialBodyId::Earth) {
        let end_year = 2000.0 + cov.end / (365.25 * SECONDS_PER_DAY);
        println!("Earth table coverage ends at: year {:.1}", end_year);
        println!(
            "Table end time: {:.2e} seconds from J2000\n",
            cov.end
        );
    }

    // Test at different time points
    let test_times = [
        (0.0, "J2000 (year 2000)"),
        (200.0 * 365.25 * SECONDS_PER_DAY, "Year 2200 (table end)"),
        (250.0 * 365.25 * SECONDS_PER_DAY, "Year 2250 (50y past)"),
        (844.0 * 365.25 * SECONDS_PER_DAY, "Year 2844 (problem case)"),
    ];

    println!("=== Earth Position at Various Times ===\n");
    println!(
        "{:25} | {:>12} | {:>12} | {:>10}",
        "Time", "X (AU)", "Y (AU)", "R (AU)"
    );
    println!("{:-<65}", "");

    for (time, label) in test_times {
        if let Some(pos) = eph.get_position_by_id(CelestialBodyId::Earth, time) {
            println!(
                "{:25} | {:12.6} | {:12.6} | {:10.6}",
                label,
                pos.x / AU_TO_METERS,
                pos.y / AU_TO_METERS,
                pos.length() / AU_TO_METERS
            );
        }
    }

    // Test orbit stability at year 2844
    println!("\n=== Orbit Stability at Year 2844 ===\n");
    let time_2844 = 844.0 * 365.25 * SECONDS_PER_DAY;
    let earth_period = 365.25 * SECONDS_PER_DAY;

    // Sample Earth position over one orbit
    let samples = 12;
    let mut positions = Vec::new();
    println!(
        "{:>8} | {:>12} | {:>12} | {:>10}",
        "Phase", "X (AU)", "Y (AU)", "R (AU)"
    );
    println!("{:-<50}", "");

    for i in 0..samples {
        let t = time_2844 + (i as f64 / samples as f64) * earth_period;
        if let Some(pos) = eph.get_position_by_id(CelestialBodyId::Earth, t) {
            positions.push(pos);
            if i % 3 == 0 {
                println!(
                    "{:>8} | {:12.6} | {:12.6} | {:10.6}",
                    format!("{}/12", i),
                    pos.x / AU_TO_METERS,
                    pos.y / AU_TO_METERS,
                    pos.length() / AU_TO_METERS
                );
            }
        }
    }

    // Compute orbit center (should be near origin for Sun-centered orbit)
    if positions.len() == samples {
        let center = positions
            .iter()
            .fold(bevy::math::DVec2::ZERO, |acc, &p| acc + p)
            / samples as f64;
        let center_offset_au = center.length() / AU_TO_METERS;

        println!("\nOrbit center offset from Sun: {:.6} AU", center_offset_au);

        // Compute min/max radius
        let radii: Vec<f64> = positions.iter().map(|p| p.length()).collect();
        let min_r = radii.iter().cloned().fold(f64::MAX, f64::min) / AU_TO_METERS;
        let max_r = radii.iter().cloned().fold(f64::MIN, f64::max) / AU_TO_METERS;

        println!("Perihelion: {:.6} AU", min_r);
        println!("Aphelion: {:.6} AU", max_r);
        println!("Eccentricity: {:.4}", (max_r - min_r) / (max_r + min_r));

        // Check if orbit is reasonable
        // Pure Kepler orbits use mean elements that differ slightly from actual positions,
        // so we use 0.05 AU threshold (much better than old constant-offset approach)
        let is_stable = center_offset_au < 0.05 && min_r > 0.95 && max_r < 1.05;
        println!(
            "\nOrbit stability check: {}",
            if is_stable { "PASS ✓" } else { "FAIL ✗" }
        );

        if !is_stable {
            println!("  - Center offset should be < 0.05 AU, got {:.6} AU", center_offset_au);
            println!("  - Perihelion should be > 0.95 AU, got {:.6} AU", min_r);
            println!("  - Aphelion should be < 1.05 AU, got {:.6} AU", max_r);
        }
    }

    // Check beyond-table indicator
    println!("\n=== Table Coverage Indicator ===\n");
    for (time, label) in test_times {
        let beyond = eph.is_beyond_table_coverage(time);
        println!("{:25} -> beyond coverage: {}", label, beyond);
    }

    println!("\n=== All Planets at Year 2844 ===\n");
    let planets = [
        CelestialBodyId::Mercury,
        CelestialBodyId::Venus,
        CelestialBodyId::Earth,
        CelestialBodyId::Mars,
        CelestialBodyId::Jupiter,
        CelestialBodyId::Saturn,
        CelestialBodyId::Uranus,
        CelestialBodyId::Neptune,
    ];

    println!(
        "{:10} | {:>12} | {:>12} | {:>10}",
        "Planet", "X (AU)", "Y (AU)", "R (AU)"
    );
    println!("{:-<52}", "");

    for planet in planets {
        if let Some(pos) = eph.get_position_by_id(planet, time_2844) {
            println!(
                "{:10} | {:12.4} | {:12.4} | {:10.4}",
                format!("{:?}", planet),
                pos.x / AU_TO_METERS,
                pos.y / AU_TO_METERS,
                pos.length() / AU_TO_METERS
            );
        }
    }

    println!("\n=== Diagnostic Complete ===");
}
