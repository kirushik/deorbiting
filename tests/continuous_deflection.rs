//! Test continuous deflection methods effectiveness.
//!
//! Verifies that Ion Beam, Laser Ablation, and Solar Sail can provide
//! sufficient combined delta-v to deflect an asteroid away from Earth.
//!
//! Run with: cargo test --test continuous_deflection

const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;
const EARTH_RADIUS_M: f64 = 6.371e6;
const G0: f64 = 9.80665; // Standard gravity for Isp calculation

/// Calculate ion beam delta-v for given parameters.
fn ion_beam_delta_v(
    thrust_n: f64,
    fuel_mass_kg: f64,
    specific_impulse: f64,
    asteroid_mass: f64,
) -> (f64, f64) {
    // Burn rate: mdot = F / (g0 × Isp)
    let mdot = thrust_n / (G0 * specific_impulse);
    let burn_time = fuel_mass_kg / mdot;

    // Acceleration: a = F / M
    let acceleration = thrust_n / asteroid_mass;

    // Delta-v: Δv = a × t
    let delta_v = acceleration * burn_time;

    (delta_v, burn_time)
}

/// Calculate laser ablation delta-v for given parameters.
fn laser_ablation_delta_v(
    power_kw: f64,
    efficiency: f64,
    mission_duration: f64,
    solar_distance_au: f64,
    asteroid_mass: f64,
) -> f64 {
    // Thrust: base is 115 N per 100 kW at 1 AU (50x boosted from 2.3 N)
    let base_thrust_per_100kw = 2.3 * 50.0; // 115 N
    let solar_efficiency = (1.0 / (solar_distance_au * solar_distance_au)).min(1.0);
    let thrust = (power_kw / 100.0) * base_thrust_per_100kw * solar_efficiency * efficiency;

    // Acceleration: a = F / M
    let acceleration = thrust / asteroid_mass;

    // Delta-v: Δv = a × t
    acceleration * mission_duration
}

/// Calculate solar sail delta-v for given parameters.
fn solar_sail_delta_v(
    sail_area_m2: f64,
    reflectivity: f64,
    mission_duration: f64,
    solar_distance_au: f64,
    asteroid_mass: f64,
) -> f64 {
    // Solar radiation pressure for perfect reflection at 1 AU: 9.08e-6 N/m²
    // Boosted 100x for gameplay: 9.08e-4 N/m²
    let srp_at_1au = 9.08e-6 * 100.0;

    // Thrust falls off with distance squared
    let distance_factor = 1.0 / (solar_distance_au * solar_distance_au);
    let thrust = srp_at_1au * sail_area_m2 * reflectivity * distance_factor;

    // Acceleration: a = F / M
    let acceleration = thrust / asteroid_mass;

    // Delta-v: Δv = a × t
    acceleration * mission_duration
}

/// Calculate required delta-v to miss Earth.
fn required_delta_v_for_miss(
    asteroid_velocity: f64,
    intercept_distance_m: f64,
    miss_distance_m: f64,
) -> f64 {
    // Lateral displacement = Δv × (D / V)
    // For miss: Δv > miss_distance × V / D
    miss_distance_m * asteroid_velocity / intercept_distance_m
}

fn main() {
    println!("=== Continuous Deflection Methods Effectiveness Test ===\n");

    // Target asteroid parameters (typical challenge scenario)
    let asteroid_mass = 3e10; // 300m asteroid, 3e10 kg
    let solar_distance_au = 1.0; // Near 1 AU

    println!("Target asteroid: 3×10¹⁰ kg (~300m diameter)");
    println!("Distance from Sun: {:.1} AU\n", solar_distance_au);

    // Ion Beam parameters (from ion_beam_default)
    let ion_thrust_n = 50_000.0;
    let ion_fuel_kg = 2_000_000.0;
    let ion_isp = 3500.0;

    let (ion_delta_v, ion_burn_time) =
        ion_beam_delta_v(ion_thrust_n, ion_fuel_kg, ion_isp, asteroid_mass);

    println!("=== Ion Beam Shepherd ===");
    println!("  Thrust: {} kN", ion_thrust_n / 1000.0);
    println!("  Fuel: {} tons", ion_fuel_kg / 1000.0);
    println!("  Specific impulse: {} s", ion_isp);
    println!("  Burn time: {:.1} days", ion_burn_time / SECONDS_PER_DAY);
    println!("  Delta-v: {:.2} m/s", ion_delta_v);
    println!();

    // Laser Ablation parameters (from laser_ablation_default)
    let laser_power_kw = 50_000.0;
    let laser_efficiency = 0.8;
    let laser_duration = 0.5 * 365.25 * SECONDS_PER_DAY; // 6 months

    let laser_delta_v = laser_ablation_delta_v(
        laser_power_kw,
        laser_efficiency,
        laser_duration,
        solar_distance_au,
        asteroid_mass,
    );

    println!("=== Laser Ablation (DE-STAR) ===");
    println!("  Power: {} MW", laser_power_kw / 1000.0);
    println!("  Efficiency: {}%", laser_efficiency * 100.0);
    println!(
        "  Mission duration: {:.0} days ({:.1} months)",
        laser_duration / SECONDS_PER_DAY,
        laser_duration / (30.0 * SECONDS_PER_DAY)
    );
    println!("  Delta-v: {:.2} m/s", laser_delta_v);
    println!();

    // Solar Sail parameters (from solar_sail_default)
    let sail_area_m2 = 10_000_000.0; // 10 km²
    let sail_reflectivity = 0.9;
    let sail_duration = 2.0 * 365.25 * SECONDS_PER_DAY; // 2 years

    let sail_delta_v = solar_sail_delta_v(
        sail_area_m2,
        sail_reflectivity,
        sail_duration,
        solar_distance_au,
        asteroid_mass,
    );

    println!("=== Solar Sail ===");
    println!("  Sail area: {} km²", sail_area_m2 / 1e6);
    println!("  Reflectivity: {}%", sail_reflectivity * 100.0);
    println!(
        "  Mission duration: {:.0} days ({:.1} years)",
        sail_duration / SECONDS_PER_DAY,
        sail_duration / (365.25 * SECONDS_PER_DAY)
    );
    println!("  Delta-v: {:.2} m/s", sail_delta_v);
    println!();

    // Combined effectiveness
    let total_delta_v = ion_delta_v + laser_delta_v + sail_delta_v;

    println!("=== Combined Effectiveness ===");
    println!("  Total delta-v from all methods: {:.2} m/s", total_delta_v);
    println!();

    // What intercept distance does this cover?
    let asteroid_velocity = 29_000.0; // m/s (typical approach velocity)
    let miss_threshold = EARTH_RADIUS_M * 2.5; // 2.5× Earth radius for safety

    println!("=== Required Δv for Earth Miss ===");
    println!(
        "  Asteroid velocity: {:.1} km/s",
        asteroid_velocity / 1000.0
    );
    println!(
        "  Required miss distance: {:.0} km (2.5× Earth radius)",
        miss_threshold / 1000.0
    );
    println!();

    let intercept_distances_au = [1.0, 0.5, 0.25, 0.1, 0.05];

    println!("  Intercept Distance | Required Δv | Time Before Impact");
    println!("  -------------------|-------------|-------------------");
    for &dist_au in &intercept_distances_au {
        let dist_m = dist_au * AU_TO_METERS;
        let time_to_earth = dist_m / asteroid_velocity;
        let required_dv = required_delta_v_for_miss(asteroid_velocity, dist_m, miss_threshold);
        let sufficient = if total_delta_v >= required_dv {
            "✓"
        } else {
            "✗"
        };

        println!(
            "  {:.2} AU ({:>6.0} km) |  {:>6.1} m/s | {:>5.0} days  {}",
            dist_au,
            dist_m / 1e9,
            required_dv,
            time_to_earth / SECONDS_PER_DAY,
            sufficient
        );
    }

    println!();

    // Calculate the threshold where our methods are effective
    let effective_distance_m = miss_threshold * asteroid_velocity / total_delta_v;
    let effective_distance_au = effective_distance_m / AU_TO_METERS;
    let effective_time_days = effective_distance_m / asteroid_velocity / SECONDS_PER_DAY;

    println!("=== Effectiveness Threshold ===");
    println!(
        "  With {:.1} m/s total Δv, deflection is effective at:",
        total_delta_v
    );
    println!(
        "    Distance: {:.3} AU ({:.0} million km)",
        effective_distance_au,
        effective_distance_m / 1e9
    );
    println!("    Time before impact: {:.0} days", effective_time_days);
    println!();

    // Test assertions
    println!("=== Test Results ===");

    let ion_ok = ion_delta_v > 1.0;
    let laser_ok = laser_delta_v > 10.0;
    let sail_ok = sail_delta_v > 5.0;
    let combined_ok = total_delta_v > 30.0;

    println!(
        "  Ion Beam provides >1 m/s: {} ({:.2} m/s)",
        if ion_ok { "PASS" } else { "FAIL" },
        ion_delta_v
    );
    println!(
        "  Laser Ablation provides >10 m/s: {} ({:.2} m/s)",
        if laser_ok { "PASS" } else { "FAIL" },
        laser_delta_v
    );
    println!(
        "  Solar Sail provides >5 m/s: {} ({:.2} m/s)",
        if sail_ok { "PASS" } else { "FAIL" },
        sail_delta_v
    );
    println!(
        "  Combined provides >30 m/s: {} ({:.2} m/s)",
        if combined_ok { "PASS" } else { "FAIL" },
        total_delta_v
    );

    let all_pass = ion_ok && laser_ok && sail_ok && combined_ok;
    println!();

    if all_pass {
        println!("✓ All tests PASSED!");
        println!("  The three continuous deflection methods together can provide");
        println!("  sufficient delta-v to deflect a 300m asteroid from Earth.");
    } else {
        println!("✗ Some tests FAILED!");
        println!("  Parameters need adjustment to provide effective deflection.");
        std::process::exit(1);
    }
}
