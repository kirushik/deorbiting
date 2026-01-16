//! Analyze planetary positions at J2000 to help design scenarios.
//!
//! Run with: cargo run --example analyze_planet_positions

use bevy::math::DVec2;
use std::collections::HashMap;

const AU_TO_METERS: f64 = 1.495978707e11;
const GM_SUN: f64 = 1.32712440018e20;
const DAY_SECONDS: f64 = 86400.0;

/// Planetary data with J2000 mean longitude
fn get_planet_j2000_data() -> HashMap<&'static str, (f64, f64, f64, f64)> {
    // (semi_major_axis_AU, eccentricity, mean_longitude_deg_at_J2000, orbital_period_days)
    let mut data = HashMap::new();
    // Mean longitude at J2000 from JPL (approximate)
    data.insert("Mercury", (0.387, 0.206, 252.25, 87.97));
    data.insert("Venus", (0.723, 0.007, 181.98, 224.70));
    data.insert("Earth", (1.000, 0.017, 100.46, 365.25));
    data.insert("Mars", (1.524, 0.093, 355.45, 686.98));
    data.insert("Jupiter", (5.203, 0.048, 34.40, 4332.59));
    data.insert("Saturn", (9.537, 0.054, 49.94, 10759.22));
    data
}

fn planet_position_j2000(name: &str, time_days: f64, data: &HashMap<&str, (f64, f64, f64, f64)>) -> DVec2 {
    let (a_au, e, mean_lon_j2000_deg, period_days) = *data.get(name).unwrap();
    let a = a_au * AU_TO_METERS;

    // Mean motion (degrees per day)
    let n = 360.0 / period_days;

    // Mean longitude at time t
    let mean_lon = (mean_lon_j2000_deg + n * time_days).to_radians();

    // For simplicity, assume argument of perihelion ≈ 0
    // Mean anomaly ≈ mean longitude for planets (roughly)
    let m = mean_lon;

    // Solve Kepler's equation
    let mut e_anomaly = m;
    for _ in 0..20 {
        let delta = (e_anomaly - e * e_anomaly.sin() - m) / (1.0 - e * e_anomaly.cos());
        e_anomaly -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }

    // True anomaly
    let true_anomaly = 2.0 * ((1.0 + e).sqrt() * (e_anomaly / 2.0).tan()).atan2((1.0 - e).sqrt());

    // Distance
    let r = a * (1.0 - e * e) / (1.0 + e * true_anomaly.cos());

    DVec2::new(r * true_anomaly.cos(), r * true_anomaly.sin())
}

fn main() {
    println!("=== PLANETARY POSITIONS ANALYSIS ===\n");

    let data = get_planet_j2000_data();

    println!("Positions at J2000 (time = 0):");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("{:10} | {:>10} | {:>10} | {:>10} | {:>8}", "Planet", "X (AU)", "Y (AU)", "R (AU)", "Angle°");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    for planet in ["Mercury", "Venus", "Earth", "Mars", "Jupiter", "Saturn"] {
        let pos = planet_position_j2000(planet, 0.0, &data);
        let r = pos.length() / AU_TO_METERS;
        let angle = pos.y.atan2(pos.x).to_degrees();
        println!("{:10} | {:10.4} | {:10.4} | {:10.4} | {:8.1}",
                 planet, pos.x / AU_TO_METERS, pos.y / AU_TO_METERS, r, angle);
    }

    // Now design the scenarios
    println!("\n\n=== SCENARIO DESIGN ===\n");

    // For Earth collision (dynamic) - this one works
    println!("1. EARTH COLLISION (dynamic computation - works correctly)");
    let earth_pos = planet_position_j2000("Earth", 0.0, &data);
    let earth_r = earth_pos.length();
    let earth_angle = earth_pos.y.atan2(earth_pos.x);
    println!("   Earth at: ({:.4}, {:.4}) AU, angle {:.1}°",
             earth_pos.x / AU_TO_METERS, earth_pos.y / AU_TO_METERS, earth_angle.to_degrees());

    // Asteroid 45° ahead
    let offset_angle = std::f64::consts::PI / 4.0;
    let asteroid_angle = earth_angle + offset_angle;
    let asteroid_pos = DVec2::new(earth_r * asteroid_angle.cos(), earth_r * asteroid_angle.sin());
    let v_circular = (GM_SUN / earth_r).sqrt();
    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
    let vel = -tangent * v_circular;

    println!("   Asteroid at: ({:.4}, {:.4}) AU, angle {:.1}°",
             asteroid_pos.x / AU_TO_METERS, asteroid_pos.y / AU_TO_METERS, asteroid_angle.to_degrees());
    println!("   Velocity: ({:.2}, {:.2}) km/s = {:.2} km/s retrograde",
             vel.x / 1000.0, vel.y / 1000.0, vel.length() / 1000.0);

    // For Jupiter Slingshot - need to approach Jupiter
    println!("\n2. JUPITER SLINGSHOT");
    let jupiter_pos = planet_position_j2000("Jupiter", 0.0, &data);
    println!("   Jupiter at J2000: ({:.4}, {:.4}) AU, angle {:.1}°",
             jupiter_pos.x / AU_TO_METERS, jupiter_pos.y / AU_TO_METERS,
             jupiter_pos.y.atan2(jupiter_pos.x).to_degrees());

    // For slingshot: approach from behind (lower orbit, moving faster)
    // Place asteroid about 1 AU behind Jupiter's current position
    let jup_angle = jupiter_pos.y.atan2(jupiter_pos.x);
    let approach_angle = jup_angle - 0.3; // ~17° behind Jupiter
    let approach_r = 4.0 * AU_TO_METERS; // Inside Jupiter's orbit
    let asteroid_pos = DVec2::new(approach_r * approach_angle.cos(), approach_r * approach_angle.sin());

    // Velocity: prograde, slightly faster than circular to catch Jupiter
    let v_circular_4au = (GM_SUN / approach_r).sqrt();
    let approach_tangent = DVec2::new(-approach_angle.sin(), approach_angle.cos());
    let vel = approach_tangent * v_circular_4au * 1.15; // Faster to climb out

    println!("   Suggested asteroid position: ({:.4}, {:.4}) AU",
             asteroid_pos.x / AU_TO_METERS, asteroid_pos.y / AU_TO_METERS);
    println!("   Suggested velocity: ({:.2}, {:.2}) km/s",
             vel.x / 1000.0, vel.y / 1000.0);

    // For Apophis flyby - close Earth approach
    println!("\n3. APOPHIS FLYBY");
    // Start closer to Earth's path
    let earth_angle = earth_pos.y.atan2(earth_pos.x);
    // Asteroid slightly outside, ahead of Earth
    let flyby_angle = earth_angle + 0.15; // ~8° ahead
    let flyby_r = 1.05 * AU_TO_METERS;
    let flyby_pos = DVec2::new(flyby_r * flyby_angle.cos(), flyby_r * flyby_angle.sin());

    // Retrograde but aimed for close pass
    let v_at_flyby = (GM_SUN / flyby_r).sqrt();
    let flyby_tangent = DVec2::new(-flyby_angle.sin(), flyby_angle.cos());
    // Add slight inward component
    let flyby_radial = DVec2::new(flyby_angle.cos(), flyby_angle.sin());
    let vel = -flyby_tangent * v_at_flyby * 0.95 - flyby_radial * 2000.0;

    println!("   Earth at: ({:.4}, {:.4}) AU", earth_pos.x / AU_TO_METERS, earth_pos.y / AU_TO_METERS);
    println!("   Suggested asteroid position: ({:.4}, {:.4}) AU",
             flyby_pos.x / AU_TO_METERS, flyby_pos.y / AU_TO_METERS);
    println!("   Suggested velocity: ({:.2}, {:.2}) km/s",
             vel.x / 1000.0, vel.y / 1000.0);

    // For Deflection Challenge - collision course with lead time
    println!("\n4. DEFLECTION CHALLENGE");
    // Place asteroid 90° ahead of Earth (6 months lead time)
    let challenge_angle = earth_angle + std::f64::consts::PI / 2.0;
    let challenge_r = earth_r; // Same orbital radius
    let challenge_pos = DVec2::new(challenge_r * challenge_angle.cos(), challenge_r * challenge_angle.sin());

    // Retrograde velocity
    let v_circular = (GM_SUN / challenge_r).sqrt();
    let challenge_tangent = DVec2::new(-challenge_angle.sin(), challenge_angle.cos());
    let vel = -challenge_tangent * v_circular;

    println!("   Asteroid at: ({:.4}, {:.4}) AU (90° ahead = ~91 days to collision)",
             challenge_pos.x / AU_TO_METERS, challenge_pos.y / AU_TO_METERS);
    println!("   Velocity: ({:.2}, {:.2}) km/s retrograde",
             vel.x / 1000.0, vel.y / 1000.0);

    // Verify by simulation
    println!("\n\n=== SIMULATING REVISED JUPITER SLINGSHOT ===\n");
    simulate_jupiter_slingshot(&data);
}

fn simulate_jupiter_slingshot(data: &HashMap<&str, (f64, f64, f64, f64)>) {
    let jupiter_pos = planet_position_j2000("Jupiter", 0.0, data);
    let jup_angle = jupiter_pos.y.atan2(jupiter_pos.x);

    // Approach from behind Jupiter
    let approach_angle = jup_angle - 0.3;
    let approach_r = 4.0 * AU_TO_METERS;
    let mut pos = DVec2::new(approach_r * approach_angle.cos(), approach_r * approach_angle.sin());

    let v_circular = (GM_SUN / approach_r).sqrt();
    let tangent = DVec2::new(-approach_angle.sin(), approach_angle.cos());
    let mut vel = tangent * v_circular * 1.15;

    let initial_speed = vel.length();

    println!("Initial: pos = ({:.2}, {:.2}) AU, speed = {:.2} km/s",
             pos.x / AU_TO_METERS, pos.y / AU_TO_METERS, initial_speed / 1000.0);

    let dt = 3600.0;
    let mut time = 0.0;
    let max_time = 1500.0 * DAY_SECONDS;
    let mut closest_jupiter = f64::MAX;
    let mut closest_time = 0.0;

    while time < max_time {
        // Simple gravity from Sun only
        let r_sq = pos.length_squared();
        let r = r_sq.sqrt();
        let acc_sun = -pos * (GM_SUN / (r_sq * r));

        // Jupiter gravity
        let jup_pos = planet_position_j2000("Jupiter", time / DAY_SECONDS, data);
        let delta = pos - jup_pos;
        let jup_r_sq = delta.length_squared();
        let jup_r = jup_r_sq.sqrt();
        let gm_jupiter = 1.898e27 * 6.67430e-11;
        let acc_jup = -delta * (gm_jupiter / (jup_r_sq * jup_r));

        let acc = acc_sun + acc_jup;

        // Velocity Verlet
        pos = pos + vel * dt + acc * (0.5 * dt * dt);
        let acc_new = {
            let r_sq = pos.length_squared();
            let r = r_sq.sqrt();
            let acc_sun = -pos * (GM_SUN / (r_sq * r));
            let jup_pos = planet_position_j2000("Jupiter", (time + dt) / DAY_SECONDS, data);
            let delta = pos - jup_pos;
            let jup_r_sq = delta.length_squared();
            let jup_r = jup_r_sq.sqrt();
            let acc_jup = -delta * (gm_jupiter / (jup_r_sq * jup_r));
            acc_sun + acc_jup
        };
        vel = vel + (acc + acc_new) * (0.5 * dt);
        time += dt;

        // Track closest approach
        let jup_dist = (pos - planet_position_j2000("Jupiter", time / DAY_SECONDS, data)).length();
        if jup_dist < closest_jupiter {
            closest_jupiter = jup_dist;
            closest_time = time;
        }

        // Print every 100 days
        let day = (time / DAY_SECONDS) as i32;
        if day % 100 == 0 && day > 0 && ((time - dt) / DAY_SECONDS) as i32 != day {
            println!("Day {:4}: pos ({:6.2}, {:6.2}) AU, speed {:.2} km/s, Jupiter dist {:.4} AU",
                     day, pos.x / AU_TO_METERS, pos.y / AU_TO_METERS,
                     vel.length() / 1000.0, jup_dist / AU_TO_METERS);
        }
    }

    let final_speed = vel.length();
    println!("\nClosest Jupiter approach: {:.4} AU at day {:.0}",
             closest_jupiter / AU_TO_METERS, closest_time / DAY_SECONDS);
    println!("Velocity change: {:.2} km/s → {:.2} km/s (Δv = {:.2} km/s)",
             initial_speed / 1000.0, final_speed / 1000.0, (final_speed - initial_speed) / 1000.0);
}
