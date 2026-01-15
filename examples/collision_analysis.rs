//! Collision analysis - understand why hits are nearly impossible
//! and test different solutions.

use bevy::math::DVec2;

const AU: f64 = 1.496e11;
const G: f64 = 6.67430e-11;
const SUN_MASS: f64 = 1.989e30;
const EARTH_RADIUS: f64 = 6.371e6; // meters
const COLLISION_MULTIPLIER: f64 = 50.0; // Danger zone multiplier
const DANGER_ZONE: f64 = EARTH_RADIUS * COLLISION_MULTIPLIER;

fn main() {
    println!("=== Collision Feasibility Analysis ===\n");

    // Earth orbital parameters
    let earth_r = AU;
    let earth_v = (G * SUN_MASS / earth_r).sqrt();
    let earth_period = 2.0 * std::f64::consts::PI * earth_r / earth_v;

    println!("Earth orbital parameters:");
    println!("  Radius: {:.3} AU", earth_r / AU);
    println!("  Velocity: {:.2} km/s", earth_v / 1000.0);
    println!("  Period: {:.1} days\n", earth_period / 86400.0);

    // Retrograde asteroid at same radius
    let relative_velocity = earth_v * 2.0; // Head-on = 2x orbital velocity
    println!("Retrograde collision scenario:");
    println!("  Relative velocity: {:.2} km/s", relative_velocity / 1000.0);

    // Time window for collision
    let crossing_time = 2.0 * EARTH_RADIUS / relative_velocity;
    println!("  Time to cross Earth diameter: {:.2} seconds", crossing_time);
    println!("  At 1-hour timestep, asteroid moves: {:.0} km", relative_velocity * 3600.0 / 1000.0);
    println!("  At 1-minute timestep, asteroid moves: {:.0} km", relative_velocity * 60.0 / 1000.0);
    println!("  Earth radius: {:.0} km\n", EARTH_RADIUS / 1000.0);

    // The problem: orbit placement precision
    println!("=== Orbit Precision Problem ===\n");

    // For two bodies on circular orbits at same radius but opposite directions
    // They meet when their angular difference = 180 degrees
    // Starting 90 degrees apart, they meet at 45 degrees each (1/8 orbit)
    let time_to_meet = earth_period / 8.0;
    println!("If asteroid at 90° ahead (retrograde):");
    println!("  Time until paths cross: {:.1} days", time_to_meet / 86400.0);

    // But "paths crossing" doesn't mean collision!
    // At exact same radius, they cross at the same point
    // But any radius difference means they miss

    // Miss distance due to radius offset
    for radius_error_km in [1.0, 10.0, 100.0, 1000.0, 10000.0] {
        let miss = radius_error_km * 1000.0; // Convert to meters
        let hits = miss < EARTH_RADIUS;
        println!("  Radius error {:.0} km -> miss by {:.0} km -> {}",
            radius_error_km, miss / 1000.0, if hits { "HIT" } else { "MISS" });
    }

    println!("\n=== Solution Analysis ===\n");

    // Solution 1: Larger collision radius
    println!("Solution 1: Larger collision detection radius");
    for multiplier in [1.0, 10.0, 50.0, 100.0] {
        let effective_radius = EARTH_RADIUS * multiplier;
        println!("  {}x radius = {:.0} km", multiplier, effective_radius / 1000.0);
    }

    // Solution 2: Interpolation
    println!("\nSolution 2: Collision interpolation during timestep");
    println!("  Check closest approach between timestep start and end");
    println!("  If closest approach < radius, register collision");

    // Solution 3: True targeting (Lambert problem or analytical)
    println!("\nSolution 3: Lambert solver for exact intercept");
    println!("  Calculate velocity to hit moving target at future position");

    // Let's try the analytical approach for circular orbit intercept
    println!("\n=== Analytical Intercept Test ===\n");

    // For head-on collision on circular orbits:
    // Earth moves CCW at omega = v/r
    // Asteroid moves CW at omega = v/r (if same radius)
    // Angular closing rate = 2*omega
    // Time to collision = angular_separation / (2 * omega)

    let omega = earth_v / earth_r; // rad/s
    println!("Angular velocity: {:.2e} rad/s", omega);
    println!("  = {:.4} deg/day", omega * 86400.0 * 180.0 / std::f64::consts::PI);

    // For collision, they need to be at exactly the same point
    // This only happens if they start at positions that, when evolved, coincide

    // True intercept: use slightly elliptical orbit
    println!("\n=== Elliptical Intercept Test ===\n");
    test_elliptical_intercept();
}

fn test_elliptical_intercept() {
    // Instead of circular retrograde, use an elliptical orbit that
    // crosses Earth's path at the right time

    // Place asteroid ahead of Earth, with velocity that creates
    // an orbit crossing Earth's position when Earth arrives

    let earth_angle = 0.0_f64; // Earth at (1 AU, 0)
    let earth_pos = DVec2::new(AU, 0.0);
    let earth_vel = DVec2::new(0.0, (G * SUN_MASS / AU).sqrt());

    // Test: asteroid at 45 degrees ahead, slightly outside Earth orbit
    // with velocity aimed to intercept
    let angles_ahead = [30.0_f64, 45.0, 60.0, 90.0, 120.0, 150.0];

    for angle_deg in angles_ahead {
        let angle = earth_angle + angle_deg.to_radians();

        // Try different initial radii
        for r_factor in [0.95, 1.0, 1.05, 1.1] {
            let r = AU * r_factor;
            let pos = DVec2::new(r * angle.cos(), r * angle.sin());

            // Calculate velocity for direct intercept of Earth's future position
            // This is a simplified approach - real Lambert solver would be better
            let time_ahead = angle_deg / 360.0 * 365.25 * 86400.0 / 2.0; // Rough estimate
            let future_earth = predict_circular_position(earth_pos, earth_vel, time_ahead);

            // Velocity to reach that point in that time
            let delta_pos = future_earth - pos;
            let direct_vel = delta_pos / time_ahead;

            // But this ignores gravity! Need to add orbital component
            // For now, use vis-viva to get orbital velocity at this radius
            let v_escape = (2.0 * G * SUN_MASS / r).sqrt();
            let v_circular = (G * SUN_MASS / r).sqrt();

            // Try retrograde circular as baseline
            let tangent = DVec2::new(-angle.sin(), angle.cos());
            let retrograde_vel = -tangent * v_circular;

            // Simulate both approaches
            let (hit_direct, time_direct, closest_direct) = simulate(pos, direct_vel);
            let (hit_retro, time_retro, closest_retro) = simulate(pos, retrograde_vel);

            if hit_direct || hit_retro || closest_direct < EARTH_RADIUS * 100.0 || closest_retro < EARTH_RADIUS * 100.0 {
                println!("{}° ahead, r={:.2} AU:", angle_deg, r_factor);
                if hit_direct {
                    println!("  Direct: HIT at {:.1} days", time_direct / 86400.0);
                } else {
                    println!("  Direct: miss by {:.0} km", closest_direct / 1000.0);
                }
                if hit_retro {
                    println!("  Retrograde: HIT at {:.1} days", time_retro / 86400.0);
                } else {
                    println!("  Retrograde: miss by {:.0} km", closest_retro / 1000.0);
                }
            }
        }
    }
}

fn predict_circular_position(pos: DVec2, vel: DVec2, dt: f64) -> DVec2 {
    let omega = vel.length() / pos.length();
    let angle = pos.y.atan2(pos.x);
    let new_angle = angle + omega * dt;
    let r = pos.length();
    DVec2::new(r * new_angle.cos(), r * new_angle.sin())
}

fn simulate(start_pos: DVec2, start_vel: DVec2) -> (bool, f64, f64) {
    let mut pos = start_pos;
    let mut vel = start_vel;
    let dt = 600.0; // 10 minute steps
    let max_time = 365.25 * 86400.0; // 1 year

    let mut closest = f64::MAX;

    let mut t = 0.0;
    while t < max_time {
        // Earth position at time t
        let earth_omega = (G * SUN_MASS / (AU * AU * AU)).sqrt();
        let earth_angle = earth_omega * t;
        let earth_pos = DVec2::new(AU * earth_angle.cos(), AU * earth_angle.sin());

        let dist = (pos - earth_pos).length();
        if dist < closest {
            closest = dist;
        }

        if dist < DANGER_ZONE {
            return (true, t, closest);
        }

        // Sun collision
        if pos.length() < 6.96e8 {
            return (false, t, closest);
        }

        // Gravity from Sun
        let r = pos.length();
        let acc = -pos.normalize() * G * SUN_MASS / (r * r);

        // Velocity Verlet
        pos = pos + vel * dt + acc * (0.5 * dt * dt);
        let r_new = pos.length();
        let acc_new = -pos.normalize() * G * SUN_MASS / (r_new * r_new);
        vel = vel + (acc + acc_new) * (0.5 * dt);

        t += dt;
    }

    (false, max_time, closest)
}
