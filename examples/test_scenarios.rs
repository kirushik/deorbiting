//! Test all scenarios with the actual physics engine.
//!
//! This example simulates each scenario using realistic physics to verify
//! they produce the expected outcomes (collision, flyby, escape, etc.).
//!
//! Run with: cargo run --example test_scenarios

use bevy::math::DVec2;
use std::collections::HashMap;

/// Physical constants
const G: f64 = 6.67430e-11;
const AU_TO_METERS: f64 = 1.495978707e11;
const DAY_SECONDS: f64 = 86400.0;

/// Sun's standard gravitational parameter (GM) - m³/s²
const GM_SUN: f64 = 1.32712440018e20;
const SUN_MASS: f64 = 1.989e30;

/// Planetary data: (semi_major_axis_AU, eccentricity, mass_kg, radius_km)
fn get_planet_data() -> HashMap<&'static str, (f64, f64, f64, f64)> {
    let mut data = HashMap::new();
    data.insert("Mercury", (0.387, 0.206, 3.301e23, 2439.7));
    data.insert("Venus", (0.723, 0.007, 4.867e24, 6051.8));
    data.insert("Earth", (1.000, 0.017, 5.972e24, 6371.0));
    data.insert("Mars", (1.524, 0.093, 6.417e23, 3389.5));
    data.insert("Jupiter", (5.203, 0.048, 1.898e27, 69911.0));
    data.insert("Saturn", (9.537, 0.054, 5.683e26, 58232.0));
    data.insert("Uranus", (19.19, 0.047, 8.681e25, 25362.0));
    data.insert("Neptune", (30.07, 0.009, 1.024e26, 24622.0));
    data
}

/// Calculate planet position at given time using simple Kepler orbit
fn planet_position(name: &str, time_seconds: f64, data: &HashMap<&str, (f64, f64, f64, f64)>) -> DVec2 {
    let (a_au, e, _mass, _radius) = data.get(name).unwrap();
    let a = a_au * AU_TO_METERS;

    // Orbital period: T = 2π * sqrt(a³/GM)
    let period = 2.0 * std::f64::consts::PI * (a.powi(3) / GM_SUN).sqrt();

    // Mean anomaly
    let n = 2.0 * std::f64::consts::PI / period; // Mean motion
    let m = n * time_seconds; // Mean anomaly

    // Solve Kepler's equation: E - e*sin(E) = M
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

/// Compute gravitational acceleration from Sun and all planets
fn compute_acceleration(pos: DVec2, time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> DVec2 {
    let mut acc = DVec2::ZERO;

    // Sun gravity
    let r_sun_sq = pos.length_squared();
    if r_sun_sq > 1.0 {
        let r_sun = r_sun_sq.sqrt();
        acc -= pos * (GM_SUN / (r_sun_sq * r_sun));
    }

    // Planet gravity
    for (name, (_, _, mass, _)) in planet_data.iter() {
        let planet_pos = planet_position(name, time, planet_data);
        let delta = pos - planet_pos;
        let r_sq = delta.length_squared();
        if r_sq > 1e6 {
            let r = r_sq.sqrt();
            let gm = G * mass;
            acc -= delta * (gm / (r_sq * r));
        }
    }

    acc
}

/// Check collision with Sun or any planet
fn check_collision(pos: DVec2, time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> Option<String> {
    // Sun collision (using ~2x visual radius for detection)
    const SUN_RADIUS: f64 = 6.96e8 * 2.0;
    if pos.length() < SUN_RADIUS {
        return Some("Sun".to_string());
    }

    // Planet collisions (using ~50x radius for detection like the actual code)
    for (name, (_, _, _, radius_km)) in planet_data.iter() {
        let planet_pos = planet_position(name, time, planet_data);
        let collision_radius = radius_km * 1000.0 * 50.0; // 50x multiplier
        if (pos - planet_pos).length() < collision_radius {
            return Some(name.to_string());
        }
    }

    None
}

/// Velocity Verlet integrator step
fn verlet_step(
    pos: &mut DVec2,
    vel: &mut DVec2,
    acc: &mut DVec2,
    time: &mut f64,
    dt: f64,
    planet_data: &HashMap<&str, (f64, f64, f64, f64)>,
) {
    // Position update
    *pos = *pos + *vel * dt + *acc * (0.5 * dt * dt);
    *time += dt;

    // New acceleration
    let acc_new = compute_acceleration(*pos, *time, planet_data);

    // Velocity update
    *vel = *vel + (*acc + acc_new) * (0.5 * dt);
    *acc = acc_new;
}

/// Scenario definition
struct Scenario {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    get_initial_state: fn(f64, &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2),
    max_days: f64,
    expected_outcome: &'static str,
}

/// Earth Collision: 45° ahead of Earth, retrograde orbit
/// Uses numerical differentiation to get Earth's actual velocity, matching the game.
fn earth_collision_state(time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2) {
    // Time offset for 45° ahead in Earth's orbit
    // Earth moves at ~0.986°/day, so 45° = ~45.6 days
    let days_for_45_degrees = 45.0 / 0.9856;
    let time_offset = days_for_45_degrees * DAY_SECONDS;

    let future_time = time + time_offset;

    // Get Earth's position 45° ahead in its orbit
    let pos = planet_position("Earth", future_time, planet_data);

    // Compute Earth's actual velocity via numerical differentiation
    // This captures both tangential and radial components
    let dt = 60.0; // 1 minute
    let pos_before = planet_position("Earth", future_time - dt, planet_data);
    let pos_after = planet_position("Earth", future_time + dt, planet_data);
    let earth_velocity = (pos_after - pos_before) / (2.0 * dt);

    // Retrograde: asteroid travels opposite to Earth
    let vel = -earth_velocity;

    (pos, vel)
}

/// Apophis flyby: close approach trajectory (DYNAMIC)
fn apophis_flyby_state(time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2) {
    let earth_pos = planet_position("Earth", time, planet_data);
    let earth_r = earth_pos.length();
    let earth_angle = earth_pos.y.atan2(earth_pos.x);

    // 5° ahead of Earth, slightly outside orbit
    let flyby_angle = earth_angle + 0.09;
    let flyby_r = earth_r * 1.02;

    let pos = DVec2::new(flyby_r * flyby_angle.cos(), flyby_r * flyby_angle.sin());

    // Retrograde velocity with slight inward component
    let v_circular = (GM_SUN / flyby_r).sqrt();
    let tangent = DVec2::new(-flyby_angle.sin(), flyby_angle.cos());
    let radial = DVec2::new(flyby_angle.cos(), flyby_angle.sin());
    let vel = -tangent * v_circular * 0.98 - radial * 1500.0;

    (pos, vel)
}

/// Jupiter slingshot (DYNAMIC) - Ahead of Jupiter, slower velocity
/// Jupiter catches up and pulls asteroid forward for velocity boost
fn jupiter_slingshot_state(time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2) {
    let jupiter_pos = planet_position("Jupiter", time, planet_data);
    let jupiter_r = jupiter_pos.length();
    let jupiter_angle = jupiter_pos.y.atan2(jupiter_pos.x);

    // Start AHEAD of Jupiter (17° ahead), at same orbital radius
    let ahead_angle = jupiter_angle + 0.3;
    let start_r = jupiter_r;
    let pos = DVec2::new(start_r * ahead_angle.cos(), start_r * ahead_angle.sin());

    // Prograde velocity, but SLOWER than circular (90%)
    // Jupiter will catch up and pull asteroid forward
    let v_circular = (GM_SUN / start_r).sqrt();
    let tangent = DVec2::new(-ahead_angle.sin(), ahead_angle.cos());
    let vel = tangent * v_circular * 0.9;

    (pos, vel)
}

/// Interstellar visitor (hyperbolic) - static is fine
fn interstellar_visitor_state(_time: f64, _planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2) {
    let pos = DVec2::new(-3.0 * AU_TO_METERS, 4.0 * AU_TO_METERS);
    let vel = DVec2::new(28_000.0, -28_000.0); // ~40 km/s
    (pos, vel)
}

/// Deflection Challenge: 91° ahead of Earth, retrograde orbit.
/// Uses numerical differentiation to get Earth's actual velocity, matching the game.
fn deflection_challenge_state(time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2) {
    // Time offset for 91° ahead in Earth's orbit → collision ~46 days
    let days_for_91_degrees = 91.0 / 0.9856;
    let time_offset = days_for_91_degrees * DAY_SECONDS;

    let future_time = time + time_offset;

    // Get Earth's position 91° ahead in its orbit
    let pos = planet_position("Earth", future_time, planet_data);

    // Compute Earth's actual velocity via numerical differentiation
    let dt = 60.0;
    let pos_before = planet_position("Earth", future_time - dt, planet_data);
    let pos_after = planet_position("Earth", future_time + dt, planet_data);
    let earth_velocity = (pos_after - pos_before) / (2.0 * dt);

    // Retrograde: asteroid travels opposite to Earth
    let vel = -earth_velocity;

    (pos, vel)
}

/// Sandbox (DYNAMIC) - near Earth, zero velocity
fn sandbox_state(time: f64, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) -> (DVec2, DVec2) {
    let earth_pos = planet_position("Earth", time, planet_data);
    let earth_angle = earth_pos.y.atan2(earth_pos.x);
    let sandbox_r = earth_pos.length() * 1.05;

    let pos = DVec2::new(sandbox_r * earth_angle.cos(), sandbox_r * earth_angle.sin());
    let vel = DVec2::ZERO;

    (pos, vel)
}

fn main() {
    println!("=== SCENARIO SIMULATION TEST ===\n");

    let planet_data = get_planet_data();

    let scenarios = vec![
        Scenario {
            id: "earth_collision",
            name: "Earth Collision Course",
            description: "Asteroid 45° ahead of Earth, retrograde → collision ~23 days",
            get_initial_state: earth_collision_state,
            max_days: 60.0,
            expected_outcome: "Collision with Earth",
        },
        Scenario {
            id: "apophis_flyby",
            name: "Apophis Flyby",
            description: "Close Earth approach (~30,000 km)",
            get_initial_state: apophis_flyby_state,
            max_days: 120.0,
            expected_outcome: "Close flyby",
        },
        Scenario {
            id: "jupiter_slingshot",
            name: "Jupiter Slingshot",
            description: "Gravity assist gaining ~10 km/s",
            get_initial_state: jupiter_slingshot_state,
            max_days: 2000.0,
            expected_outcome: "Jupiter encounter + velocity boost",
        },
        Scenario {
            id: "interstellar_visitor",
            name: "Interstellar Visitor",
            description: "Hyperbolic escape trajectory",
            get_initial_state: interstellar_visitor_state,
            max_days: 500.0,
            expected_outcome: "Escape (E > 0)",
        },
        Scenario {
            id: "deflection_challenge",
            name: "Deflection Challenge",
            description: "Collision course with ~46 day warning (91° ahead)",
            get_initial_state: deflection_challenge_state,
            max_days: 80.0,
            expected_outcome: "Collision with Earth (~46 days)",
        },
        Scenario {
            id: "sandbox",
            name: "Sandbox",
            description: "Zero velocity near Earth orbit",
            get_initial_state: sandbox_state,
            max_days: 180.0,
            expected_outcome: "Falls toward Sun",
        },
    ];

    for scenario in &scenarios {
        simulate_scenario(scenario, &planet_data);
    }

    println!("\n=== ALL SCENARIOS TESTED ===");
}

fn simulate_scenario(scenario: &Scenario, planet_data: &HashMap<&str, (f64, f64, f64, f64)>) {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("SCENARIO: {} ({})", scenario.name, scenario.id);
    println!("Description: {}", scenario.description);
    println!("Expected: {}", scenario.expected_outcome);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let start_time = 0.0; // J2000 epoch

    // Get initial state
    let (mut pos, mut vel) = (scenario.get_initial_state)(start_time, planet_data);

    println!("Initial state:");
    println!("  Position: ({:.4}, {:.4}) AU", pos.x / AU_TO_METERS, pos.y / AU_TO_METERS);
    println!("  Distance from Sun: {:.4} AU", pos.length() / AU_TO_METERS);
    println!("  Velocity: ({:.2}, {:.2}) km/s", vel.x / 1000.0, vel.y / 1000.0);
    println!("  Speed: {:.2} km/s", vel.length() / 1000.0);

    // Earth info
    let earth_pos = planet_position("Earth", start_time, planet_data);
    let initial_earth_dist = (pos - earth_pos).length();
    println!("  Distance to Earth: {:.4} AU ({:.0} km)",
             initial_earth_dist / AU_TO_METERS,
             initial_earth_dist / 1000.0);

    // Orbital energy
    let r = pos.length();
    let v = vel.length();
    let specific_energy = 0.5 * v * v - GM_SUN / r;
    println!("\nOrbital parameters:");
    println!("  Specific energy: {:.2e} J/kg ({})",
             specific_energy,
             if specific_energy > 0.0 { "HYPERBOLIC" } else { "BOUND" });

    if specific_energy < 0.0 {
        let semi_major_axis = -GM_SUN / (2.0 * specific_energy);
        let h = pos.x * vel.y - pos.y * vel.x;
        let e_sq = 1.0 + (2.0 * specific_energy * h * h) / (GM_SUN * GM_SUN);
        let eccentricity = e_sq.max(0.0).sqrt();
        let period_days = 2.0 * std::f64::consts::PI * (semi_major_axis.powi(3) / GM_SUN).sqrt() / DAY_SECONDS;

        println!("  Semi-major axis: {:.4} AU", semi_major_axis / AU_TO_METERS);
        println!("  Eccentricity: {:.4}", eccentricity);
        println!("  Period: {:.1} days ({:.2} years)", period_days, period_days / 365.25);
    }

    // Run simulation
    println!("\n--- Running simulation for {:.0} days ---\n", scenario.max_days);

    let mut acc = compute_acceleration(pos, start_time, planet_data);
    let mut time = start_time;
    let max_time = start_time + scenario.max_days * DAY_SECONDS;

    let dt = 3600.0; // 1 hour timestep

    let mut closest_earth_dist = f64::MAX;
    let mut closest_earth_time = 0.0;
    let mut collision_body: Option<String> = None;
    let initial_speed = vel.length();

    let mut last_printed_day = -1;

    // Print header
    let print_interval = if scenario.max_days > 500.0 { 50 } else if scenario.max_days > 100.0 { 10 } else { 5 };
    println!("Day    | Sun (AU) | Earth (AU) | Earth (km)    | Speed (km/s)");
    println!("-------|----------|------------|---------------|-------------");

    while time < max_time && collision_body.is_none() {
        verlet_step(&mut pos, &mut vel, &mut acc, &mut time, dt, planet_data);

        // Check collision
        if let Some(body) = check_collision(pos, time, planet_data) {
            let elapsed = (time - start_time) / DAY_SECONDS;
            println!("\n*** COLLISION with {} at day {:.2}! ***", body, elapsed);
            collision_body = Some(body);
            break;
        }

        // Track Earth distance
        let earth_pos = planet_position("Earth", time, planet_data);
        let earth_dist = (pos - earth_pos).length();
        if earth_dist < closest_earth_dist {
            closest_earth_dist = earth_dist;
            closest_earth_time = time;
        }

        // Print daily status
        let current_day = ((time - start_time) / DAY_SECONDS) as i32;
        if current_day > last_printed_day && current_day % print_interval == 0 {
            let sun_dist = pos.length();
            let speed = vel.length();
            println!("{:5}  | {:8.4} | {:10.4} | {:13.0} | {:11.2}",
                     current_day,
                     sun_dist / AU_TO_METERS,
                     earth_dist / AU_TO_METERS,
                     earth_dist / 1000.0,
                     speed / 1000.0);
            last_printed_day = current_day;
        }

        // Check escape
        if pos.length() > 100.0 * AU_TO_METERS {
            println!("\n*** ESCAPED SOLAR SYSTEM (>100 AU) ***");
            break;
        }
    }

    // Final summary
    println!("\n--- Summary ---");
    let elapsed_days = (time - start_time) / DAY_SECONDS;
    println!("Simulated: {:.1} days", elapsed_days);
    println!("Closest Earth approach: {:.6} AU ({:.0} km) at day {:.1}",
             closest_earth_dist / AU_TO_METERS,
             closest_earth_dist / 1000.0,
             (closest_earth_time - start_time) / DAY_SECONDS);

    // Classify outcome
    if collision_body.is_some() {
        println!("\nOUTCOME: COLLISION with {}", collision_body.unwrap());
    } else {
        let final_energy = 0.5 * vel.length_squared() - GM_SUN / pos.length();
        if final_energy > 0.0 {
            println!("\nOUTCOME: ESCAPE TRAJECTORY");
        } else {
            println!("\nOUTCOME: BOUND ORBIT (no collision in simulation)");

            // Check if it was supposed to collide
            if scenario.id == "earth_collision" || scenario.id == "deflection_challenge" {
                println!("  !!! PROBLEM: Expected collision with Earth but none occurred !!!");
                println!("  Closest approach was {:.0} km (Earth radius ~6371 km, detection ~318,550 km)",
                         closest_earth_dist / 1000.0);
            }
        }
    }

    // Speed change for Jupiter slingshot
    if scenario.id == "jupiter_slingshot" {
        let final_speed = vel.length();
        let delta_v = final_speed - initial_speed;
        println!("\nVelocity change: {:.2} km/s → {:.2} km/s (Δv = {:.2} km/s)",
                 initial_speed / 1000.0, final_speed / 1000.0, delta_v / 1000.0);
    }

    println!("\n");
}
