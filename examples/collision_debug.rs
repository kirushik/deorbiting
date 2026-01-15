//! Debug collision detection to verify system ordering fix works.

use bevy::math::DVec2;

const AU: f64 = 1.496e11;
const G: f64 = 6.67430e-11;
const SUN_MASS: f64 = 1.989e30;
const EARTH_RADIUS: f64 = 6.371e6;
const COLLISION_MULTIPLIER: f64 = 50.0;
const DANGER_ZONE: f64 = EARTH_RADIUS * COLLISION_MULTIPLIER;

fn main() {
    println!("=== Collision Detection Debug ===\n");
    println!("Earth radius: {:.0} km", EARTH_RADIUS / 1000.0);
    println!("Danger zone (50x): {:.0} km\n", DANGER_ZONE / 1000.0);

    // Simulate the initial asteroid scenario: 45° ahead, retrograde
    let earth_angle = 0.0_f64;
    let earth_pos = DVec2::new(AU, 0.0);

    // Asteroid at 45° ahead
    let offset_angle = std::f64::consts::PI / 4.0;
    let asteroid_angle = earth_angle + offset_angle;
    let asteroid_r = AU;

    let asteroid_pos = DVec2::new(
        asteroid_r * asteroid_angle.cos(),
        asteroid_r * asteroid_angle.sin(),
    );

    // Retrograde circular velocity
    let gm_sun = G * SUN_MASS;
    let v_circular = (gm_sun / asteroid_r).sqrt();
    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());
    let asteroid_vel = -tangent * v_circular;

    println!("Initial asteroid:");
    println!("  Position: ({:.3} AU, {:.3} AU)", asteroid_pos.x / AU, asteroid_pos.y / AU);
    println!("  Velocity: {:.2} km/s (retrograde)", asteroid_vel.length() / 1000.0);
    println!("  Earth at: ({:.3} AU, {:.3} AU)", earth_pos.x / AU, earth_pos.y / AU);

    // Simulate
    let mut pos = asteroid_pos;
    let mut vel = asteroid_vel;
    let dt = 60.0; // 1 minute steps for accurate collision detection
    let max_time = 100.0 * 86400.0; // 100 days

    let mut closest_approach = f64::MAX;
    let mut closest_time = 0.0;

    let mut t = 0.0;
    while t < max_time {
        // Earth position at time t (circular orbit)
        let earth_omega = (gm_sun / (AU * AU * AU)).sqrt();
        let earth_angle_t = earth_omega * t;
        let earth_pos_t = DVec2::new(AU * earth_angle_t.cos(), AU * earth_angle_t.sin());

        let dist = (pos - earth_pos_t).length();

        if dist < closest_approach {
            closest_approach = dist;
            closest_time = t;
        }

        // Check collision
        if dist < DANGER_ZONE {
            println!("\n✓ COLLISION DETECTED at t = {:.1} days", t / 86400.0);
            println!("  Distance to Earth center: {:.0} km", dist / 1000.0);
            println!("  Relative velocity: {:.2} km/s", (vel - earth_pos_t.normalize() * v_circular).length() / 1000.0);
            return;
        }

        // Check Sun collision
        if pos.length() < 6.96e8 {
            println!("\n✗ HIT SUN at t = {:.1} days", t / 86400.0);
            return;
        }

        // Gravity from Sun
        let r = pos.length();
        let acc = -pos.normalize() * gm_sun / (r * r);

        // Velocity Verlet
        pos = pos + vel * dt + acc * (0.5 * dt * dt);
        let r_new = pos.length();
        let acc_new = -pos.normalize() * gm_sun / (r_new * r_new);
        vel = vel + (acc + acc_new) * (0.5 * dt);

        t += dt;
    }

    println!("\n✗ NO COLLISION after {:.0} days", max_time / 86400.0);
    println!("  Closest approach: {:.0} km at t = {:.1} days", closest_approach / 1000.0, closest_time / 86400.0);

    if closest_approach < DANGER_ZONE * 2.0 {
        println!("  (This was very close - might miss due to timestep)");
    }
}
