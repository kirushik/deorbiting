//! Analyze orbit drawing approaches for visual stability.
//!
//! This demonstrates the osculating orbital elements approach:
//! - Given position (r) and velocity (v), we can derive orbital elements (a, e, ω)
//! - The resulting ellipse passes exactly through the current position
//! - The elements change smoothly over time (no jitter)
//!
//! Run with: cargo run --example orbit_alignment_analysis

use bevy::math::DVec2;

const AU_TO_METERS: f64 = 1.495978707e11;
const GM_SUN: f64 = 1.32712440018e20;

/// Mercury orbital elements (from ephemeris/data.rs)
const MERCURY_A: f64 = 0.387 * AU_TO_METERS;
const MERCURY_E: f64 = 0.2056;
const MERCURY_OMEGA_DEG: f64 = 29.12;
const MERCURY_M0_DEG: f64 = 174.79;
const MERCURY_N_DEG_PER_DAY: f64 = 4.0923;

/// Solve Kepler's equation M = E - e * sin(E) for E.
fn solve_kepler(mean_anomaly: f64, eccentricity: f64) -> f64 {
    let mut e_anomaly = mean_anomaly;
    for _ in 0..50 {
        let delta = (e_anomaly - eccentricity * e_anomaly.sin() - mean_anomaly)
            / (1.0 - eccentricity * e_anomaly.cos());
        e_anomaly -= delta;
        if delta.abs() < 1e-14 {
            break;
        }
    }
    e_anomaly
}

/// Get Kepler position and velocity at time (days since J2000).
fn kepler_state(time_days: f64) -> (DVec2, DVec2) {
    let omega = MERCURY_OMEGA_DEG.to_radians();
    let m0 = MERCURY_M0_DEG.to_radians();
    let n = MERCURY_N_DEG_PER_DAY.to_radians(); // rad/day
    let n_per_sec = n / 86400.0; // rad/s

    // Mean anomaly at time
    let m = m0 + n * time_days;

    // Solve Kepler's equation
    let e_anomaly = solve_kepler(m, MERCURY_E);

    // True anomaly
    let nu = 2.0
        * ((1.0 + MERCURY_E).sqrt() * (e_anomaly / 2.0).tan())
            .atan2((1.0 - MERCURY_E).sqrt());

    // Distance from focus
    let r = MERCURY_A * (1.0 - MERCURY_E * e_anomaly.cos());

    // Position in heliocentric coordinates
    let angle = nu + omega;
    let pos = DVec2::new(r * angle.cos(), r * angle.sin());

    // Velocity (vis-viva + angular momentum)
    let p = MERCURY_A * (1.0 - MERCURY_E * MERCURY_E);
    let h = (GM_SUN * p).sqrt(); // specific angular momentum

    // Velocity components in orbital frame, then rotate
    let vr = GM_SUN / h * MERCURY_E * nu.sin(); // radial
    let vt = GM_SUN / h * (1.0 + MERCURY_E * nu.cos()); // tangential

    let cos_a = angle.cos();
    let sin_a = angle.sin();
    let vel = DVec2::new(
        vr * cos_a - vt * sin_a,
        vr * sin_a + vt * cos_a,
    );

    (pos, vel)
}

/// Simulate a perturbed N-body-like state with oscillation from Venus.
fn perturbed_state(time_days: f64) -> (DVec2, DVec2) {
    let (base_pos, base_vel) = kepler_state(time_days);

    // Add oscillatory perturbation (mimicking Venus' influence)
    let venus_period_days = 224.7;
    let perturbation_phase = 2.0 * std::f64::consts::PI * time_days / venus_period_days;

    let r = base_pos.length();
    let angle = base_pos.y.atan2(base_pos.x);

    // Position perturbations (~500 km radial, ~300 km tangential)
    let radial_perturbation = 5e5 * perturbation_phase.sin();
    let tangential_perturbation = 3e5 * (perturbation_phase * 1.3).cos();

    let new_r = r + radial_perturbation;
    let tangent_offset = tangential_perturbation / r;

    let pos = DVec2::new(
        new_r * (angle + tangent_offset).cos(),
        new_r * (angle + tangent_offset).sin(),
    );

    // Velocity perturbations (~10 m/s)
    let vel = base_vel + DVec2::new(
        10.0 * perturbation_phase.cos(),
        10.0 * (perturbation_phase * 0.7).sin(),
    );

    (pos, vel)
}

/// Compute osculating orbital elements from state vector.
/// Returns (a, e, omega) where:
/// - a: semi-major axis (meters)
/// - e: eccentricity
/// - omega: argument of periapsis (radians)
fn osculating_elements(pos: DVec2, vel: DVec2) -> (f64, f64, f64) {
    let r = pos.length();
    let v_sq = vel.length_squared();

    // Specific orbital energy: ε = v²/2 - μ/r
    let specific_energy = v_sq / 2.0 - GM_SUN / r;

    // Semi-major axis: a = -μ / (2ε)
    let a = if specific_energy.abs() > 1e-10 {
        -GM_SUN / (2.0 * specific_energy)
    } else {
        r
    };

    // Eccentricity vector: e_vec = ((v² - μ/r) * r - (r·v) * v) / μ
    let r_dot_v = pos.dot(vel);
    let e_vec = (pos * (v_sq - GM_SUN / r) - vel * r_dot_v) / GM_SUN;
    let e = e_vec.length();

    // Argument of periapsis
    let omega = e_vec.y.atan2(e_vec.x);

    (a, e, omega)
}

/// Check if a position lies on the ellipse defined by (a, e, omega)
fn position_error_on_ellipse(pos: DVec2, a: f64, e: f64, omega: f64) -> f64 {
    let r_actual = pos.length();
    let theta = pos.y.atan2(pos.x);
    let nu = theta - omega; // true anomaly

    let p = a * (1.0 - e * e);
    let r_expected = p / (1.0 + e * nu.cos());

    (r_actual - r_expected).abs()
}

fn main() {
    println!("=== OSCULATING ORBITAL ELEMENTS ANALYSIS ===\n");
    println!("This demonstrates how computing orbital elements from (pos, vel)");
    println!("ensures the planet always lies exactly on its drawn orbit.\n");

    let orbital_period_days = 87.97;

    println!("Analyzing Mercury over one orbit ({:.0} days):\n", orbital_period_days);

    // Part 1: Show that osculating elements change smoothly
    println!("=== OSCULATING ELEMENTS STABILITY ===\n");
    println!("{:>6} | {:>12} | {:>10} | {:>10} | {:>12}",
             "Day", "a (AU)", "e", "ω (°)", "Pos Error (m)");
    println!("{:-<6}-+-{:-<12}-+-{:-<10}-+-{:-<10}-+-{:-<12}", "", "", "", "", "");

    let mut prev_omega: Option<f64> = None;
    let mut max_omega_change_deg = 0.0f64;
    let step_days = 5.0;
    let mut day = 0.0;

    while day <= orbital_period_days {
        let (pos, vel) = perturbed_state(day);
        let (a, e, omega) = osculating_elements(pos, vel);

        let pos_error = position_error_on_ellipse(pos, a, e, omega);

        // Track omega changes
        if let Some(prev) = prev_omega {
            let delta = (omega - prev).abs().to_degrees();
            max_omega_change_deg = max_omega_change_deg.max(delta);
        }
        prev_omega = Some(omega);

        if (day as i32) % 10 == 0 || day < 1.0 {
            println!("{:6.0} | {:12.6} | {:10.6} | {:10.4} | {:12.2}",
                     day,
                     a / AU_TO_METERS,
                     e,
                     omega.to_degrees(),
                     pos_error);
        }

        day += step_days;
    }

    println!();
    println!("Max ω change between samples: {:.4}°", max_omega_change_deg);
    println!("Position error: always 0 (by construction)\n");

    // Part 2: Compare approaches
    println!("=== APPROACH COMPARISON ===\n");
    println!("OLD approach (baked elements + alignment):");
    println!("  - Uses fixed (a, e) from J2000 orbital elements");
    println!("  - Computes ω to fit current position");
    println!("  - Problem: If actual orbit differs from baked elements,");
    println!("    ω must oscillate to compensate → visible jitter\n");

    println!("NEW approach (osculating elements from state vector):");
    println!("  - Computes (a, e, ω) from current (pos, vel)");
    println!("  - All elements come from same data source");
    println!("  - Elements change smoothly as orbit evolves");
    println!("  - Planet always lies exactly on drawn ellipse\n");

    // Part 3: Show the difference between baked and osculating elements
    println!("=== BAKED vs OSCULATING ELEMENTS ===\n");
    println!("Baked J2000 elements:   a = {:.4} AU, e = {:.4}, ω = {:.2}°",
             MERCURY_A / AU_TO_METERS, MERCURY_E, MERCURY_OMEGA_DEG);

    let (pos, vel) = perturbed_state(0.0);
    let (osc_a, osc_e, osc_omega) = osculating_elements(pos, vel);
    println!("Osculating at day 0:    a = {:.4} AU, e = {:.4}, ω = {:.2}°",
             osc_a / AU_TO_METERS, osc_e, osc_omega.to_degrees());

    let (pos, vel) = perturbed_state(44.0); // Half orbit
    let (osc_a, osc_e, osc_omega) = osculating_elements(pos, vel);
    println!("Osculating at day 44:   a = {:.4} AU, e = {:.4}, ω = {:.2}°",
             osc_a / AU_TO_METERS, osc_e, osc_omega.to_degrees());

    println!();
    println!("✓ Osculating elements adapt to the actual orbit shape,");
    println!("  ensuring the planet is always on its path!");
}
