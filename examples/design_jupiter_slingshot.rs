//! Design a proper Jupiter gravity assist trajectory.
//!
//! Run with: cargo run --example design_jupiter_slingshot

use bevy::math::DVec2;
use std::collections::HashMap;

const AU_TO_METERS: f64 = 1.495978707e11;
const DAY_SECONDS: f64 = 86400.0;
const GM_SUN: f64 = 1.32712440018e20;
const G: f64 = 6.67430e-11;

fn get_planet_data() -> HashMap<&'static str, (f64, f64, f64, f64)> {
    let mut data = HashMap::new();
    data.insert("Jupiter", (5.203, 0.048, 1.898e27, 69911.0));
    data.insert("Earth", (1.000, 0.017, 5.972e24, 6371.0));
    data
}

fn planet_position(
    name: &str,
    time_seconds: f64,
    data: &HashMap<&str, (f64, f64, f64, f64)>,
) -> DVec2 {
    let (a_au, e, _mass, _radius) = data.get(name).unwrap();
    let a = a_au * AU_TO_METERS;
    let period = 2.0 * std::f64::consts::PI * (a.powi(3) / GM_SUN).sqrt();
    let n = 2.0 * std::f64::consts::PI / period;
    let m = n * time_seconds;

    let mut e_anomaly = m;
    for _ in 0..20 {
        let delta = (e_anomaly - e * e_anomaly.sin() - m) / (1.0 - e * e_anomaly.cos());
        e_anomaly -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }

    let true_anomaly = 2.0 * ((1.0 + e).sqrt() * (e_anomaly / 2.0).tan()).atan2((1.0 - e).sqrt());
    let r = a * (1.0 - e * e) / (1.0 + e * true_anomaly.cos());
    DVec2::new(r * true_anomaly.cos(), r * true_anomaly.sin())
}

fn compute_acceleration(
    pos: DVec2,
    time: f64,
    planet_data: &HashMap<&str, (f64, f64, f64, f64)>,
) -> DVec2 {
    let mut acc = DVec2::ZERO;
    let r_sun_sq = pos.length_squared();
    if r_sun_sq > 1.0 {
        let r_sun = r_sun_sq.sqrt();
        acc -= pos * (GM_SUN / (r_sun_sq * r_sun));
    }

    for (name, (_, _, mass, _)) in planet_data.iter() {
        let planet_pos = planet_position(name, time, planet_data);
        let delta = pos - planet_pos;
        let r_sq = delta.length_squared();
        if r_sq > 1e6 {
            let r = r_sq.sqrt();
            acc -= delta * (G * mass / (r_sq * r));
        }
    }
    acc
}

fn main() {
    println!("=== JUPITER SLINGSHOT TRAJECTORY DESIGN ===\n");

    let planet_data = get_planet_data();

    // Jupiter's position at t=0
    let jupiter_pos_0 = planet_position("Jupiter", 0.0, &planet_data);
    let jupiter_r = jupiter_pos_0.length();
    let jupiter_angle = jupiter_pos_0.y.atan2(jupiter_pos_0.x);

    println!("Jupiter at t=0:");
    println!(
        "  Position: ({:.4}, {:.4}) AU",
        jupiter_pos_0.x / AU_TO_METERS,
        jupiter_pos_0.y / AU_TO_METERS
    );
    println!("  Distance from Sun: {:.4} AU", jupiter_r / AU_TO_METERS);
    println!("  Angle: {:.1}°", jupiter_angle.to_degrees());

    // Jupiter's orbital velocity
    let v_jup = (GM_SUN / jupiter_r).sqrt();
    println!("  Orbital velocity: {:.2} km/s", v_jup / 1000.0);

    // For a gravity assist to GAIN velocity:
    // - Asteroid must approach from BEHIND Jupiter (trailing side)
    // - Pass around Jupiter's trailing hemisphere
    // - Get slingshot boost in Jupiter's direction of motion

    // Strategy: Place asteroid AHEAD of Jupiter in a slightly eccentric orbit
    // that will be caught up by Jupiter. Asteroid moving slower → Jupiter catches it
    // → asteroid passes behind Jupiter → gains velocity

    // Let's try: asteroid at 5 AU, slightly AHEAD of Jupiter, moving slower
    // This means Jupiter "catches up" and the asteroid passes behind it

    // Test several approaches
    println!("\n=== TESTING DIFFERENT APPROACHES ===\n");

    // Approach 1: Ahead of Jupiter, slower velocity
    println!("--- Approach 1: Ahead of Jupiter, slower velocity ---");
    test_trajectory(
        jupiter_angle + 0.3, // 17° ahead of Jupiter
        5.0 * AU_TO_METERS,  // Same orbit radius
        0.9,                 // Slower than circular
        true,                // Prograde
        &planet_data,
    );

    // Approach 2: Behind Jupiter, faster velocity (current approach)
    println!("\n--- Approach 2: Behind Jupiter, faster velocity ---");
    test_trajectory(
        jupiter_angle - 0.5, // 29° behind Jupiter
        4.0 * AU_TO_METERS,  // Inside orbit
        1.2,                 // Faster than circular
        true,                // Prograde
        &planet_data,
    );

    // Approach 3: Direct intercept from inner solar system
    println!("\n--- Approach 3: Direct intercept from inner system ---");
    test_direct_intercept(&planet_data);

    // Approach 4: Ahead of Jupiter, retrograde (counter-intuitive but let's see)
    println!("\n--- Approach 4: Slightly ahead, but aimed toward Jupiter ---");
    test_aimed_approach(&planet_data);
}

fn test_trajectory(
    angle: f64,
    radius: f64,
    velocity_factor: f64,
    prograde: bool,
    planet_data: &HashMap<&str, (f64, f64, f64, f64)>,
) {
    let pos = DVec2::new(radius * angle.cos(), radius * angle.sin());
    let v_circular = (GM_SUN / radius).sqrt();
    let tangent = DVec2::new(-angle.sin(), angle.cos());
    let vel = if prograde {
        tangent * v_circular * velocity_factor
    } else {
        -tangent * v_circular * velocity_factor
    };

    println!(
        "Start: ({:.4}, {:.4}) AU, speed {:.2} km/s",
        pos.x / AU_TO_METERS,
        pos.y / AU_TO_METERS,
        vel.length() / 1000.0
    );

    simulate(pos, vel, planet_data, 2000.0);
}

fn test_direct_intercept(planet_data: &HashMap<&str, (f64, f64, f64, f64)>) {
    // Start from 2 AU, aim for Jupiter's position in ~500 days
    let start_time = 0.0;
    let transfer_time = 500.0 * DAY_SECONDS;

    let jupiter_future = planet_position("Jupiter", transfer_time, planet_data);
    let jupiter_angle_future = jupiter_future.y.atan2(jupiter_future.x);

    // Start position: 2 AU, behind where Jupiter will be
    let start_angle = jupiter_angle_future - 1.0; // Behind Jupiter's future position
    let start_r = 2.0 * AU_TO_METERS;
    let pos = DVec2::new(start_r * start_angle.cos(), start_r * start_angle.sin());

    // Velocity: aim to reach Jupiter's future position
    // For Hohmann-like transfer to 5.2 AU:
    let v_perihelion = (GM_SUN * (2.0 / start_r - 1.0 / (3.6 * AU_TO_METERS))).sqrt();
    let tangent = DVec2::new(-start_angle.sin(), start_angle.cos());
    let vel = tangent * v_perihelion;

    println!(
        "Start: ({:.4}, {:.4}) AU, speed {:.2} km/s",
        pos.x / AU_TO_METERS,
        pos.y / AU_TO_METERS,
        vel.length() / 1000.0
    );
    println!(
        "Jupiter in 500 days: ({:.4}, {:.4}) AU",
        jupiter_future.x / AU_TO_METERS,
        jupiter_future.y / AU_TO_METERS
    );

    simulate(pos, vel, planet_data, 1000.0);
}

fn test_aimed_approach(planet_data: &HashMap<&str, (f64, f64, f64, f64)>) {
    // Place asteroid so it will encounter Jupiter from behind
    let jupiter_pos = planet_position("Jupiter", 0.0, planet_data);
    let jupiter_r = jupiter_pos.length();
    let jupiter_angle = jupiter_pos.y.atan2(jupiter_pos.x);

    // Jupiter's velocity direction (prograde tangent)
    let jupiter_vel_dir = DVec2::new(-jupiter_angle.sin(), jupiter_angle.cos());

    // Place asteroid 0.5 AU "behind" Jupiter (opposite to its velocity)
    let offset = -jupiter_vel_dir * 0.5 * AU_TO_METERS;
    let pos = jupiter_pos + offset;

    // Aim velocity toward Jupiter's position + some lead
    let lead_time = 100.0 * DAY_SECONDS;
    let jupiter_future = planet_position("Jupiter", lead_time, planet_data);
    let to_jupiter = (jupiter_future - pos).normalize();

    // Speed: enough to reach Jupiter (~15 km/s)
    let vel = to_jupiter * 18000.0;

    println!(
        "Start: ({:.4}, {:.4}) AU, speed {:.2} km/s",
        pos.x / AU_TO_METERS,
        pos.y / AU_TO_METERS,
        vel.length() / 1000.0
    );
    println!(
        "Jupiter now: ({:.4}, {:.4}) AU",
        jupiter_pos.x / AU_TO_METERS,
        jupiter_pos.y / AU_TO_METERS
    );

    simulate(pos, vel, planet_data, 500.0);
}

fn simulate(
    mut pos: DVec2,
    mut vel: DVec2,
    planet_data: &HashMap<&str, (f64, f64, f64, f64)>,
    max_days: f64,
) {
    let initial_speed = vel.length();
    let mut time = 0.0;
    let dt = 3600.0;
    let max_time = max_days * DAY_SECONDS;

    let mut acc = compute_acceleration(pos, time, planet_data);
    let mut closest_jupiter = f64::MAX;
    let mut closest_time = 0.0;

    while time < max_time {
        // Verlet step
        pos = pos + vel * dt + acc * (0.5 * dt * dt);
        time += dt;
        let acc_new = compute_acceleration(pos, time, planet_data);
        vel = vel + (acc + acc_new) * (0.5 * dt);
        acc = acc_new;

        // Track Jupiter distance
        let jupiter_pos = planet_position("Jupiter", time, planet_data);
        let jupiter_dist = (pos - jupiter_pos).length();
        if jupiter_dist < closest_jupiter {
            closest_jupiter = jupiter_dist;
            closest_time = time;
        }

        // Check for very close approach
        if jupiter_dist < 0.1 * AU_TO_METERS {
            let day = time / DAY_SECONDS;
            println!(
                "  Day {:4.0}: CLOSE APPROACH {:.6} AU ({:.0} Jupiter radii)",
                day,
                jupiter_dist / AU_TO_METERS,
                jupiter_dist / (69911.0 * 1000.0)
            );
        }
    }

    let final_speed = vel.length();
    let delta_v = final_speed - initial_speed;

    println!("Results:");
    println!(
        "  Closest Jupiter: {:.4} AU at day {:.0}",
        closest_jupiter / AU_TO_METERS,
        closest_time / DAY_SECONDS
    );
    println!(
        "  Velocity: {:.2} → {:.2} km/s (Δv = {:+.2} km/s)",
        initial_speed / 1000.0,
        final_speed / 1000.0,
        delta_v / 1000.0
    );
    println!("  {} boost", if delta_v > 0.0 { "✓ Got" } else { "✗ No" });
}
