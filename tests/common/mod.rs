//! Common test utilities for integration tests.

use bevy::math::DVec2;
use deorbiting::types::{AU_TO_METERS, GM_SUN, SECONDS_PER_DAY};

/// Create a circular orbit state at the given distance from the Sun.
pub fn circular_orbit(distance_au: f64) -> (DVec2, DVec2) {
    let r = distance_au * AU_TO_METERS;
    let v = (GM_SUN / r).sqrt();
    (DVec2::new(r, 0.0), DVec2::new(0.0, v))
}

/// Create an elliptical orbit state at perihelion.
pub fn elliptical_orbit(perihelion_au: f64, eccentricity: f64) -> (DVec2, DVec2) {
    let r_p = perihelion_au * AU_TO_METERS;
    let a = r_p / (1.0 - eccentricity);
    let v = (GM_SUN * (2.0 / r_p - 1.0 / a)).sqrt();
    (DVec2::new(r_p, 0.0), DVec2::new(0.0, v))
}

/// Compute specific orbital energy.
pub fn orbital_energy(pos: DVec2, vel: DVec2) -> f64 {
    let r = pos.length();
    let v = vel.length();
    0.5 * v * v - GM_SUN / r
}

/// Compute specific angular momentum (2D scalar).
pub fn angular_momentum(pos: DVec2, vel: DVec2) -> f64 {
    pos.x * vel.y - pos.y * vel.x
}

/// Compute orbital period for elliptical orbit.
pub fn orbital_period(semi_major_axis: f64) -> f64 {
    use std::f64::consts::TAU;
    TAU * (semi_major_axis.powi(3) / GM_SUN).sqrt()
}

/// Simulate using Velocity Verlet for a given duration.
pub fn simulate_verlet(
    mut pos: DVec2,
    mut vel: DVec2,
    duration: f64,
    num_steps: usize,
) -> (DVec2, DVec2) {
    let dt = duration / num_steps as f64;

    for _ in 0..num_steps {
        let r = pos.length();
        let acc = -GM_SUN / (r * r * r) * pos;

        let new_pos = pos + vel * dt + acc * (0.5 * dt * dt);
        let new_r = new_pos.length();
        let new_acc = -GM_SUN / (new_r * new_r * new_r) * new_pos;
        let new_vel = vel + (acc + new_acc) * (0.5 * dt);

        pos = new_pos;
        vel = new_vel;
    }

    (pos, vel)
}

/// Convert seconds to days.
pub fn seconds_to_days(seconds: f64) -> f64 {
    seconds / SECONDS_PER_DAY
}
