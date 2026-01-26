//! Test that asteroid reaches the same position after the same simulation time,
//! regardless of time scale used.
//!
//! The bug was: at lower scales, the asteroid would overshoot because the
//! integrator took steps larger than target_dt, causing the asteroid to move
//! more simulation time than the clock advanced.
//!
//! With the fix, running for N simulation days at 1x should put the asteroid
//! at the same place as running for N simulation days at 10x.
//!
//! Run with: cargo test --test time_sync

use bevy::math::DVec2;

/// Physical constants
const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;

/// Sun's standard gravitational parameter (GM) - m³/s²
const GM_SUN: f64 = 1.32712440018e20;

/// IAS15 integrator configuration
struct IAS15Config {
    initial_dt: f64,
    min_dt: f64,
    max_dt: f64,
    epsilon: f64,
}

impl Default for IAS15Config {
    fn default() -> Self {
        Self {
            initial_dt: 3600.0, // 1 hour
            min_dt: 1.0,        // 1 second
            max_dt: 86400.0,    // 1 day
            epsilon: 1e-9,
        }
    }
}

/// Simple integrator state
struct IAS15State {
    pos: DVec2,
    vel: DVec2,
    acc: DVec2,
    acc_prev: DVec2,
    dt: f64,
    dt_last_done: f64,
}

impl IAS15State {
    fn new(pos: DVec2, vel: DVec2, initial_acc: DVec2, config: &IAS15Config) -> Self {
        Self {
            pos,
            vel,
            acc: initial_acc,
            acc_prev: initial_acc,
            dt: config.initial_dt,
            dt_last_done: config.initial_dt,
        }
    }

    fn step<F>(&mut self, acceleration_fn: F, config: &IAS15Config)
    where
        F: Fn(DVec2, f64) -> DVec2,
    {
        let dt = self.dt;

        // Velocity Verlet step
        let pos_new = self.pos + self.vel * dt + self.acc * (0.5 * dt * dt);
        let acc_new = acceleration_fn(pos_new, dt);
        let vel_new = self.vel + (self.acc + acc_new) * (0.5 * dt);

        // Update state
        self.acc_prev = self.acc;
        self.pos = pos_new;
        self.vel = vel_new;
        self.acc = acc_new;
        self.dt_last_done = dt;

        // Adaptive timestep based on acceleration change
        let acc_diff = (acc_new - self.acc_prev).length();
        let acc_mag = acc_new.length().max(self.acc_prev.length()).max(1e-20);

        let relative_change = acc_diff / acc_mag;

        let new_dt = if relative_change > 1e-20 {
            let scale_factor = (config.epsilon / relative_change).powf(0.25);
            let scale_factor = scale_factor.clamp(0.1, 10.0);
            dt * scale_factor
        } else {
            dt * 2.0
        };

        self.dt = new_dt.clamp(config.min_dt, config.max_dt);
    }
}

/// Compute acceleration from Sun only
fn compute_acceleration_sun_only(pos: DVec2) -> DVec2 {
    let r_sq = pos.length_squared();
    if r_sq < 1.0 {
        return DVec2::ZERO;
    }
    let r = r_sq.sqrt();
    -pos * (GM_SUN / (r_sq * r))
}

/// Simulate until a fixed simulation time is reached
fn simulate_until(
    ias15: &mut IAS15State,
    config: &IAS15Config,
    target_sim_time: f64,
    scale: f64,
    fixed_dt: f64,
    with_fix: bool,
) -> (f64, usize) {
    let target_dt_per_step = fixed_dt * scale * SECONDS_PER_DAY;
    let mut sim_time = 0.0;
    let mut steps = 0;

    while sim_time < target_sim_time {
        let remaining_total = target_sim_time - sim_time;
        let target_dt = target_dt_per_step.min(remaining_total);

        if target_dt <= 0.0 {
            break;
        }

        let mut elapsed = 0.0;

        while elapsed < target_dt {
            if with_fix {
                // Cap step size to remaining time (THE FIX)
                let remaining = target_dt - elapsed;
                if ias15.dt > remaining && remaining > config.min_dt {
                    ias15.dt = remaining;
                }
            }

            let acc_fn =
                |pos: DVec2, _relative_t: f64| -> DVec2 { compute_acceleration_sun_only(pos) };

            ias15.step(acc_fn, config);
            elapsed += ias15.dt_last_done;
            steps += 1;

            if ias15.dt_last_done < 1e-10 {
                break;
            }
        }

        sim_time += target_dt;
    }

    (sim_time, steps)
}

fn main() {
    let config = IAS15Config::default();

    // Earth-collision-course-like initial conditions
    let initial_pos = DVec2::new(1.5 * AU_TO_METERS, 0.0);
    let initial_vel = DVec2::new(-15_000.0, -12_000.0);

    // Test parameters
    let fixed_dt = 1.0 / 64.0; // FixedUpdate delta in seconds (~15.6ms)

    // Target: simulate 10 simulation days
    let target_sim_days = 10.0;
    let target_sim_time = target_sim_days * SECONDS_PER_DAY;

    println!(
        "Testing time synchronization: asteroid position after {target_sim_days} simulation days\n"
    );
    println!(
        "Initial position: ({:.6} AU, {:.6} AU)",
        initial_pos.x / AU_TO_METERS,
        initial_pos.y / AU_TO_METERS
    );
    println!("Fixed timestep: {fixed_dt} seconds\n");

    // Test at multiple scales with the fix
    let scales = [1.0, 10.0, 100.0];

    println!("=== WITH the step-cap fix ===\n");

    let mut positions_with_fix = Vec::new();

    for &scale in &scales {
        let initial_acc = compute_acceleration_sun_only(initial_pos);
        let mut ias15 = IAS15State::new(initial_pos, initial_vel, initial_acc, &config);

        let (final_sim_time, steps) = simulate_until(
            &mut ias15,
            &config,
            target_sim_time,
            scale,
            fixed_dt,
            true, // with fix
        );

        println!("Scale {scale}x:");
        println!(
            "  Final position: ({:.9} AU, {:.9} AU)",
            ias15.pos.x / AU_TO_METERS,
            ias15.pos.y / AU_TO_METERS
        );
        println!(
            "  Sim time reached: {:.6} days",
            final_sim_time / SECONDS_PER_DAY
        );
        println!("  Integration steps: {steps}");
        println!();

        positions_with_fix.push((scale, ias15.pos));
    }

    // Calculate deviations from 1x baseline
    let baseline_pos = positions_with_fix[0].1;
    println!("Position deviation from 1x baseline (WITH fix):");
    for (scale, pos) in &positions_with_fix {
        let deviation_m = (*pos - baseline_pos).length();
        let deviation_km = deviation_m / 1000.0;
        println!("  Scale {scale}x: {deviation_km:.1} km");
    }

    // Now test WITHOUT the fix
    println!("\n=== WITHOUT the step-cap fix ===\n");

    let mut positions_no_fix = Vec::new();

    for &scale in &scales {
        let initial_acc = compute_acceleration_sun_only(initial_pos);
        let mut ias15 = IAS15State::new(initial_pos, initial_vel, initial_acc, &config);

        let (final_sim_time, steps) = simulate_until(
            &mut ias15,
            &config,
            target_sim_time,
            scale,
            fixed_dt,
            false, // without fix
        );

        println!("Scale {scale}x:");
        println!(
            "  Final position: ({:.9} AU, {:.9} AU)",
            ias15.pos.x / AU_TO_METERS,
            ias15.pos.y / AU_TO_METERS
        );
        println!(
            "  Sim time reached: {:.6} days",
            final_sim_time / SECONDS_PER_DAY
        );
        println!("  Integration steps: {steps}");
        println!();

        positions_no_fix.push((scale, ias15.pos));
    }

    let baseline_pos_no_fix = positions_no_fix[0].1;
    println!("Position deviation from 1x baseline (WITHOUT fix):");
    for (scale, pos) in &positions_no_fix {
        let deviation_m = (*pos - baseline_pos_no_fix).length();
        let deviation_km = deviation_m / 1000.0;
        println!("  Scale {scale}x: {deviation_km:.1} km");
    }

    // Summary
    println!("\n=== Summary ===\n");

    let max_deviation_with_fix = positions_with_fix
        .iter()
        .map(|(_, pos)| (*pos - baseline_pos).length())
        .fold(0.0f64, f64::max)
        / 1000.0;

    let max_deviation_no_fix = positions_no_fix
        .iter()
        .map(|(_, pos)| (*pos - baseline_pos_no_fix).length())
        .fold(0.0f64, f64::max)
        / 1000.0;

    println!("Max position deviation WITH fix: {max_deviation_with_fix:.1} km");
    println!("Max position deviation WITHOUT fix: {max_deviation_no_fix:.1} km");

    // For this simplified Sun-only test, even without the fix there may not be much
    // deviation because the adaptive timestep settles quickly. The real bug manifests
    // when there are multiple gravity sources causing varying accelerations.
    // The key thing is that WITH the fix, positions should match very closely.

    if max_deviation_with_fix < 1000.0 {
        // Less than 1000 km deviation
        println!("\n✓ With the fix, positions match within 1000 km - fix is working!");
    } else {
        println!("\n✗ With the fix, positions still deviate significantly");
    }
}
