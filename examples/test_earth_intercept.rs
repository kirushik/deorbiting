//! Test script to analyze Earth intercept calculations.
//!
//! This script tests different intercept calculation approaches and shows
//! whether they result in Earth collision or not.

use bevy::math::DVec2;

const AU: f64 = 1.495978707e11; // meters
const G: f64 = 6.67430e-11;
const SUN_MASS: f64 = 1.989e30;
const SUN_GM: f64 = G * SUN_MASS;
const EARTH_ORBITAL_VEL: f64 = 29_780.0; // m/s, approximately

fn main() {
    println!("=== Earth Intercept Analysis ===\n");

    // Earth's approximate position and velocity at some time
    // Let's say Earth is at (1 AU, 0) moving in +Y direction
    let earth_pos = DVec2::new(AU, 0.0);
    let earth_vel = DVec2::new(0.0, EARTH_ORBITAL_VEL);

    println!(
        "Earth position: ({:.4} AU, {:.4} AU)",
        earth_pos.x / AU,
        earth_pos.y / AU
    );
    println!(
        "Earth velocity: ({:.2} km/s, {:.2} km/s)",
        earth_vel.x / 1000.0,
        earth_vel.y / 1000.0
    );
    println!();

    // Test the BROKEN approach (what we currently have)
    println!("--- BROKEN: Linear intercept through Sun ---");
    test_broken_approach(earth_pos);

    // Test approach 1: Hohmann-like transfer
    println!("\n--- Approach 1: Hohmann-like transfer from higher orbit ---");
    test_hohmann_approach(earth_pos, earth_vel);

    // Test approach 2: Direct radial approach
    println!("\n--- Approach 2: Radial infall ---");
    test_radial_infall(earth_pos);

    // Test approach 3: Retrograde collision
    println!("\n--- Approach 3: Head-on collision (retrograde) ---");
    test_retrograde_approach(earth_pos, earth_vel);

    // Test approach 4: From outside, curved approach
    println!("\n--- Approach 4: Elliptical from 1.5 AU ---");
    test_elliptical_approach(earth_pos, earth_vel);

    // Test approach 5: Crossing orbit
    println!("\n--- Approach 5: Crossing orbit from 1.2 AU ---");
    test_crossing_orbit(earth_pos, earth_vel);

    // Test approach 6: Slow retrograde
    println!("\n--- Approach 6: Slow retrograde at 1.1 AU ---");
    test_slow_retrograde(earth_pos, earth_vel);

    // Test the NEW approach (what we're implementing)
    println!("\n--- NEW APPROACH: Retrograde at 1.05 AU, 45° ahead ---");
    test_new_approach(earth_pos);
}

fn test_new_approach(earth_pos: DVec2) {
    println!("  Testing multiple configurations:\n");

    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    // Try different offsets and radii
    let configs = [
        (30.0_f64, 1.0_f64), // 30° ahead, same radius
        (60.0, 1.0),         // 60° ahead, same radius
        (90.0, 1.0),         // 90° ahead, same radius
        (45.0, 1.05),        // 45° ahead, slightly outside
        (30.0, 0.95),        // 30° ahead, slightly inside
        (-30.0, 1.0),        // 30° behind, same radius (retrograde catches up)
    ];

    for (offset_deg, r_factor) in configs {
        let offset_angle = offset_deg.to_radians();
        let asteroid_angle = angle_now + offset_angle;
        let asteroid_r = r_factor * AU;

        let asteroid_pos = DVec2::new(
            asteroid_r * asteroid_angle.cos(),
            asteroid_r * asteroid_angle.sin(),
        );

        let v_circular = (SUN_GM / asteroid_r).sqrt();
        let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
        let vel = -tangent * v_circular; // Retrograde

        let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, &earth_pos_fn);
        let result = if hit {
            format!("HIT at {:.1} days", time / 86400.0)
        } else {
            "NO HIT".to_string()
        };

        println!(
            "    {:.0}° {:+.2} AU: vel={:.1} km/s -> {}",
            offset_deg,
            r_factor,
            vel.length() / 1000.0,
            result
        );
    }
}

fn simulate_trajectory(
    start_pos: DVec2,
    start_vel: DVec2,
    earth_pos_fn: impl Fn(f64) -> DVec2,
) -> (bool, f64, DVec2) {
    let dt = 3600.0; // 1 hour steps
    let max_time = 365.0 * 86400.0; // 1 year max

    let mut pos = start_pos;
    let mut vel = start_vel;
    let mut t = 0.0;

    let earth_radius = 6.371e6; // meters
    let sun_radius = 6.96e8; // meters

    while t < max_time {
        // Check Earth collision
        let earth_pos = earth_pos_fn(t);
        let dist_to_earth = (pos - earth_pos).length();
        if dist_to_earth < earth_radius * 10.0 {
            // Within 10 Earth radii
            return (true, t, pos);
        }

        // Check Sun collision
        if pos.length() < sun_radius {
            println!("  HIT SUN at t = {:.1} days!", t / 86400.0);
            return (false, t, pos);
        }

        // Check escape
        if pos.length() > 10.0 * AU {
            println!("  ESCAPED at t = {:.1} days", t / 86400.0);
            return (false, t, pos);
        }

        // Gravity from Sun
        let r = pos.length();
        let acc = -pos.normalize_or_zero() * SUN_GM / (r * r);

        // Simple Euler integration
        vel = vel + acc * dt;
        pos = pos + vel * dt;
        t += dt;
    }

    println!("  NO COLLISION after 1 year");
    (false, t, pos)
}

fn test_broken_approach(earth_pos: DVec2) {
    // Current broken approach: linear intercept
    let intercept_time = 180.0 * 86400.0;

    // Earth's future position (approximate - assume circular orbit)
    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let angle_future = angle_now + angular_vel * intercept_time;
    let earth_future = DVec2::new(AU * angle_future.cos(), AU * angle_future.sin());

    // Asteroid opposite to Earth's future position
    let asteroid_dir = -earth_future.normalize_or_zero();
    let asteroid_pos = asteroid_dir * 1.5 * AU;

    // Velocity toward Earth's future position
    let delta = earth_future - asteroid_pos;
    let vel = delta / intercept_time * 1.05;

    println!(
        "  Asteroid starts at ({:.4} AU, {:.4} AU)",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU
    );
    println!(
        "  Velocity: ({:.2} km/s, {:.2} km/s) = {:.2} km/s",
        vel.x / 1000.0,
        vel.y / 1000.0,
        vel.length() / 1000.0
    );
    println!(
        "  Aiming at Earth future pos: ({:.4} AU, {:.4} AU)",
        earth_future.x / AU,
        earth_future.y / AU
    );
    println!("  PROBLEM: Trajectory passes through Sun at origin!");

    // Simulate
    let earth_pos_fn = |t: f64| {
        let angle = angle_now + angular_vel * t;
        DVec2::new(AU * angle.cos(), AU * angle.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}

fn test_hohmann_approach(earth_pos: DVec2, earth_vel: DVec2) {
    // Place asteroid at 1.3 AU at same angle as Earth
    // Give it velocity for transfer orbit with perihelion at Earth's orbit
    let angle = earth_pos.y.atan2(earth_pos.x);
    let r1 = 1.3 * AU; // Start at 1.3 AU
    let r2 = 1.0 * AU; // Target perihelion at Earth's orbit

    let asteroid_pos = DVec2::new(r1 * angle.cos(), r1 * angle.sin());

    // For elliptical orbit with aphelion r1 and perihelion r2:
    // v_aphelion = sqrt(GM * 2*r2 / (r1 * (r1 + r2)))
    let v_transfer = (SUN_GM * 2.0 * r2 / (r1 * (r1 + r2))).sqrt();

    // Velocity should be tangential (perpendicular to radius)
    let tangent = DVec2::new(-angle.sin(), angle.cos());
    let vel = tangent * v_transfer;

    // Circular velocity at r1 for comparison
    let v_circular = (SUN_GM / r1).sqrt();

    println!(
        "  Asteroid at ({:.4} AU, {:.4} AU)",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU
    );
    println!(
        "  Transfer velocity: {:.2} km/s (circular would be {:.2} km/s)",
        v_transfer / 1000.0,
        v_circular / 1000.0
    );

    // Simulate
    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}

fn test_radial_infall(earth_pos: DVec2) {
    // Place asteroid directly "above" Earth at 1.5 AU, let it fall
    // This won't work well in 2D since there's no "above"
    // Instead, place it at 1.5 AU at same angle, zero tangential velocity
    let angle = earth_pos.y.atan2(earth_pos.x);
    let asteroid_pos = DVec2::new(1.5 * AU * angle.cos(), 1.5 * AU * angle.sin());

    // Zero initial velocity - just let it fall
    let vel = DVec2::ZERO;

    println!(
        "  Asteroid at ({:.4} AU, {:.4} AU)",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU
    );
    println!("  Starting from rest - pure radial infall");

    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}

fn test_retrograde_approach(earth_pos: DVec2, earth_vel: DVec2) {
    // Head-on collision: place asteroid on Earth's orbit but moving opposite direction
    // Offset by some angle so they'll meet
    let angle = earth_pos.y.atan2(earth_pos.x);
    let offset_angle = std::f64::consts::PI / 6.0; // 30 degrees ahead
    let asteroid_angle = angle + offset_angle;

    let asteroid_pos = DVec2::new(AU * asteroid_angle.cos(), AU * asteroid_angle.sin());

    // Retrograde circular velocity
    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
    let v_circular = (SUN_GM / AU).sqrt();
    let vel = -tangent * v_circular; // Negative = retrograde

    println!(
        "  Asteroid at ({:.4} AU, {:.4} AU), {:.1}° ahead of Earth",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU,
        offset_angle.to_degrees()
    );
    println!("  Retrograde velocity: {:.2} km/s", v_circular / 1000.0);

    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}

fn test_crossing_orbit(earth_pos: DVec2, _earth_vel: DVec2) {
    // Place asteroid at 1.2 AU, slightly behind Earth
    // Give it prograde velocity for an eccentric orbit that crosses Earth's orbit
    let angle = earth_pos.y.atan2(earth_pos.x);
    let asteroid_angle = angle - std::f64::consts::PI / 3.0; // 60 degrees behind

    let r_start = 1.2 * AU;
    let asteroid_pos = DVec2::new(
        r_start * asteroid_angle.cos(),
        r_start * asteroid_angle.sin(),
    );

    // Give it velocity slightly less than circular - will fall inward
    // Targeting perihelion around 0.9 AU
    let r_perihelion = 0.9 * AU;
    let v_apo = (SUN_GM * 2.0 * r_perihelion / (r_start * (r_start + r_perihelion))).sqrt();

    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
    let vel = tangent * v_apo;

    println!(
        "  Asteroid at ({:.4} AU, {:.4} AU), 60° behind Earth",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU
    );
    println!(
        "  Prograde, eccentric orbit (perihelion {:.2} AU)",
        r_perihelion / AU
    );
    println!("  Velocity: {:.2} km/s", v_apo / 1000.0);

    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}

fn test_slow_retrograde(earth_pos: DVec2, _earth_vel: DVec2) {
    // Retrograde but at slightly higher orbit - will take longer
    let angle = earth_pos.y.atan2(earth_pos.x);
    let offset_angle = std::f64::consts::PI / 2.0; // 90 degrees ahead
    let asteroid_angle = angle + offset_angle;

    let r_start = 1.1 * AU; // Slightly outside Earth's orbit
    let asteroid_pos = DVec2::new(
        r_start * asteroid_angle.cos(),
        r_start * asteroid_angle.sin(),
    );

    // Retrograde at this radius
    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
    let v_circular = (SUN_GM / r_start).sqrt();
    let vel = -tangent * v_circular * 0.9; // Slightly slower than circular

    println!(
        "  Asteroid at ({:.4} AU, {:.4} AU), 90° ahead, 1.1 AU out",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU
    );
    println!(
        "  Slow retrograde velocity: {:.2} km/s",
        vel.length() / 1000.0
    );

    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}

fn test_elliptical_approach(earth_pos: DVec2, _earth_vel: DVec2) {
    // Place asteroid at 1.5 AU, 90 degrees behind Earth
    // Give it velocity that creates an ellipse crossing Earth's orbit
    let angle = earth_pos.y.atan2(earth_pos.x);
    let asteroid_angle = angle - std::f64::consts::PI / 2.0; // 90 degrees behind

    let asteroid_pos = DVec2::new(
        1.5 * AU * asteroid_angle.cos(),
        1.5 * AU * asteroid_angle.sin(),
    );

    // For an ellipse from 1.5 AU to perihelion at 0.8 AU:
    let r1 = 1.5 * AU;
    let r2 = 0.8 * AU;
    let v_apo = (SUN_GM * 2.0 * r2 / (r1 * (r1 + r2))).sqrt();

    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
    let vel = tangent * v_apo;

    println!(
        "  Asteroid at ({:.4} AU, {:.4} AU)",
        asteroid_pos.x / AU,
        asteroid_pos.y / AU
    );
    println!("  Elliptical orbit, perihelion at {:.2} AU", r2 / AU);
    println!("  Velocity: {:.2} km/s", v_apo / 1000.0);

    let angular_vel = EARTH_ORBITAL_VEL / AU;
    let angle_now = earth_pos.y.atan2(earth_pos.x);
    let earth_pos_fn = |t: f64| {
        let ang = angle_now + angular_vel * t;
        DVec2::new(AU * ang.cos(), AU * ang.sin())
    };

    let (hit, time, _) = simulate_trajectory(asteroid_pos, vel, earth_pos_fn);
    if hit {
        println!("  Result: HIT EARTH at t = {:.1} days", time / 86400.0);
    }
}
