//! Debug example to analyze deflection effectiveness requirements.
//!
//! Run with: cargo run --example debug_deflection_effectiveness

const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;
const EARTH_RADIUS_M: f64 = 6.371e6;

fn main() {
    println!("=== Deflection Effectiveness Analysis ===\n");

    // Analysis: How much delta-v is needed to miss Earth?
    //
    // If an asteroid is heading toward Earth at velocity V,
    // and we apply perpendicular delta-v of Δv at distance D from Earth,
    // the lateral displacement when reaching Earth is:
    //   displacement = Δv × (D / V)
    //
    // For Earth miss (displacement > Earth radius + margin):
    //   Δv > (Earth_radius + margin) × V / D

    let asteroid_velocity = 29_000.0; // m/s (typical)
    let safety_margin = EARTH_RADIUS_M * 1.5; // 1.5× Earth radius for safety
    let miss_threshold = EARTH_RADIUS_M + safety_margin;

    println!("Asteroid velocity: {:.1} km/s", asteroid_velocity / 1000.0);
    println!("Earth radius: {:.0} km", EARTH_RADIUS_M / 1000.0);
    println!(
        "Required miss distance: {:.0} km\n",
        miss_threshold / 1000.0
    );

    println!("=== Required Δv for Earth miss at different intercept distances ===\n");

    let intercept_distances_au = [1.0, 0.5, 0.25, 0.1, 0.05, 0.01];

    for &dist_au in &intercept_distances_au {
        let dist_m = dist_au * AU_TO_METERS;
        let time_to_earth = dist_m / asteroid_velocity;
        let required_dv = miss_threshold * asteroid_velocity / dist_m;

        println!(
            "{:.2} AU ({:.0e} km): need {:.1} m/s Δv (intercept {:.0} days before impact)",
            dist_au,
            dist_m / 1000.0,
            required_dv,
            time_to_earth / SECONDS_PER_DAY
        );
    }

    println!("\n=== Current Payload Effectiveness ===\n");

    // Current dart() parameters: mass=200,000 kg, β=40 (gameplay-boosted)
    // Formula: Δv = β × m × v_rel / M_asteroid

    let dart_mass = 200_000.0; // kg
    let dart_beta = 40.0;
    let relative_velocity = 30_000.0; // m/s (typical high-speed intercept)

    // Current nuclear parameters: 12,000 kt (12 MT) (gameplay-boosted)
    // Formula: Δv = 0.30 × (yield/100) × (3e10/M_asteroid)
    let nuclear_yield = 12_000.0; // kt

    let asteroid_masses = [
        ("Small (1e9 kg, ~80m)", 1e9),
        ("Medium (3e10 kg, ~300m)", 3e10),
        ("Large (1e11 kg, ~500m)", 1e11),
        ("Very Large (5e11 kg, ~800m)", 5e11),
    ];

    println!("Kinetic Impactor (dart): {} kg, β={}", dart_mass, dart_beta);
    for (name, mass) in &asteroid_masses {
        let dv = dart_beta * dart_mass * relative_velocity / mass;
        println!("  {}: {:.2} m/s", name, dv);
    }

    println!("\nNuclear Standoff (2 MT):");
    for (name, mass) in &asteroid_masses {
        let reference_dv = 0.30;
        let reference_yield = 100.0;
        let reference_mass = 3e10;
        let dv = reference_dv * (nuclear_yield / reference_yield) * (reference_mass / mass);
        println!("  {}: {:.2} m/s", name, dv);
    }

    println!("\n=== Recommendation ===\n");

    // For an asteroid at 0.1 AU (common intercept), we need ~20 m/s
    // With 3-5 launches each providing ~10-20 m/s, we should be safe

    println!("For effective deflection with 3-5 launches:\n");

    let target_dv_per_launch = 15.0; // m/s - achievable with boosted parameters
    let num_launches = 4;
    let total_dv = target_dv_per_launch * num_launches as f64;

    println!(
        "Target: {} m/s per launch × {} launches = {} m/s total",
        target_dv_per_launch, num_launches, total_dv
    );

    // What parameters achieve this?
    let medium_asteroid_mass = 3e10;

    // For kinetic: Δv = β × m × v_rel / M
    // Solving for β×m: β×m = Δv × M / v_rel
    let required_beta_mass = target_dv_per_launch * medium_asteroid_mass / relative_velocity;
    println!("\nFor {} m/s against 300m asteroid:", target_dv_per_launch);
    println!(
        "  Kinetic needs β×m = {:.0} kg (e.g., 100t×{}β or 200t×{}β)",
        required_beta_mass,
        (required_beta_mass / 100_000.0).round(),
        (required_beta_mass / 200_000.0).round()
    );

    // For nuclear: Δv = 0.30 × (yield/100) × (3e10/M)
    // Solving for yield: yield = Δv × 100 × M / (0.30 × 3e10)
    let required_yield = target_dv_per_launch * 100.0 * medium_asteroid_mass / (0.30 * 3e10);
    println!(
        "  Nuclear needs {:.0} kt ({:.1} MT)",
        required_yield,
        required_yield / 1000.0
    );

    println!("\n=== Multi-Launch Effectiveness (3-5 launches) ===\n");

    // Show effectiveness of multiple launches
    let dv_dart_medium = dart_beta * dart_mass * relative_velocity / 3e10;
    let dv_nuclear_medium = 0.30 * (nuclear_yield / 100.0);

    println!("Against 300m asteroid (3e10 kg):\n");
    println!(
        "3 kinetic impacts: {:.1} m/s (effective at 0.25 AU / 15 days)",
        dv_dart_medium * 3.0
    );
    println!(
        "1 nuclear: {:.1} m/s (effective at 0.1 AU / 6 days)",
        dv_nuclear_medium
    );
    println!(
        "4 kinetic impacts: {:.1} m/s (effective at 0.1 AU / 6 days)",
        dv_dart_medium * 4.0
    );
    println!(
        "3 kinetic + 2 nuclear: {:.1} m/s (effective at 0.05 AU / 3 days - emergency)",
        dv_dart_medium * 3.0 + dv_nuclear_medium * 2.0
    );
}
