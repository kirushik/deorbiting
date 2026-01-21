//! Benchmark for trajectory prediction performance.
//!
//! Measures per-step cost breakdown and full trajectory times for various prediction lengths.
//! This establishes baselines for the optimization work.
//!
//! Run with: cargo run --example benchmark_prediction --release
//!
//! Key metrics:
//! - Per-step costs: ephemeris lookup, gravity computation, dominant body, collision check
//! - Full trajectory times: 1, 5, 12, 20 year predictions
//! - Memory usage: trajectory point storage

use bevy::math::DVec2;
use std::time::Instant;

/// Physical constants
const AU: f64 = 1.495978707e11;
const DAY_SECONDS: f64 = 86400.0;
const YEAR_SECONDS: f64 = 365.25 * DAY_SECONDS;
const GM_SUN: f64 = 1.32712440018e20;
const G: f64 = 6.67430e-11;

/// Number of gravity sources (Sun + 8 planets)
const GRAVITY_SOURCE_COUNT: usize = 9;

/// Collision multiplier for planets
const COLLISION_MULTIPLIER: f64 = 50.0;

/// Benchmark configuration
const WARMUP_ITERATIONS: usize = 3;
const BENCHMARK_ITERATIONS: usize = 10;

/// Planet data: (semi-major axis AU, eccentricity, mass kg, radius m, GM m³/s²)
#[derive(Clone, Copy)]
struct PlanetData {
    a: f64,      // semi-major axis in meters
    e: f64,      // eccentricity
    gm: f64,     // gravitational parameter
    radius: f64, // radius in meters
    period: f64, // orbital period in seconds
}

/// Full gravity source data (what we want to fetch once per timestep)
#[derive(Clone, Copy)]
struct GravitySourceFull {
    id: usize,             // 0=Sun, 1=Mercury, etc.
    pos: DVec2,            // position in meters
    gm: f64,               // GM in m³/s²
    collision_radius: f64, // effective collision radius (with multiplier)
}

/// Get planet data for all 8 planets
fn get_planet_data() -> [PlanetData; 8] {
    // Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune
    let raw = [
        (0.387, 0.206, 3.301e23, 2.4397e6),
        (0.723, 0.007, 4.867e24, 6.0518e6),
        (1.000, 0.017, 5.972e24, 6.371e6),
        (1.524, 0.093, 6.417e23, 3.3895e6),
        (5.203, 0.048, 1.898e27, 6.9911e7),
        (9.537, 0.054, 5.683e26, 5.8232e7),
        (19.19, 0.047, 8.681e25, 2.5362e7),
        (30.07, 0.009, 1.024e26, 2.4622e7),
    ];

    let mut result = [PlanetData {
        a: 0.0,
        e: 0.0,
        gm: 0.0,
        radius: 0.0,
        period: 0.0,
    }; 8];

    for (i, (a_au, e, mass, radius)) in raw.iter().enumerate() {
        let a = a_au * AU;
        result[i] = PlanetData {
            a,
            e: *e,
            gm: G * mass,
            radius: *radius,
            period: 2.0 * std::f64::consts::PI * (a.powi(3) / GM_SUN).sqrt(),
        };
    }
    result
}

/// Simulate Kepler position lookup (represents ephemeris interpolation cost)
fn kepler_position(data: &PlanetData, time: f64) -> DVec2 {
    let n = 2.0 * std::f64::consts::PI / data.period;
    let m = n * time;

    // Newton-Raphson for Kepler's equation (simulates real ephemeris work)
    let mut e_anomaly = m;
    for _ in 0..10 {
        let delta = (e_anomaly - data.e * e_anomaly.sin() - m) / (1.0 - data.e * e_anomaly.cos());
        e_anomaly -= delta;
        if delta.abs() < 1e-12 {
            break;
        }
    }

    let true_anomaly =
        2.0 * ((1.0 + data.e).sqrt() * (e_anomaly / 2.0).tan()).atan2((1.0 - data.e).sqrt());
    let r = data.a * (1.0 - data.e * data.e) / (1.0 + data.e * true_anomaly.cos());

    DVec2::new(r * true_anomaly.cos(), r * true_anomaly.sin())
}

/// CURRENT APPROACH: Separate function calls with redundant ephemeris lookups
mod current {
    use super::*;

    /// Get gravity sources (position + GM only)
    pub fn get_gravity_sources(
        planets: &[PlanetData; 8],
        time: f64,
    ) -> [(DVec2, f64); GRAVITY_SOURCE_COUNT] {
        let mut sources = [(DVec2::ZERO, 0.0); GRAVITY_SOURCE_COUNT];

        // Sun at origin
        sources[0] = (DVec2::ZERO, GM_SUN);

        // Planets
        for i in 0..8 {
            let pos = kepler_position(&planets[i], time);
            sources[i + 1] = (pos, planets[i].gm);
        }

        sources
    }

    /// Get gravity sources with IDs (for dominant body detection)
    pub fn get_gravity_sources_with_id(
        planets: &[PlanetData; 8],
        time: f64,
    ) -> [(usize, DVec2, f64); GRAVITY_SOURCE_COUNT] {
        let mut sources = [(0, DVec2::ZERO, 0.0); GRAVITY_SOURCE_COUNT];

        // Sun at origin
        sources[0] = (0, DVec2::ZERO, GM_SUN);

        // Planets - DUPLICATE ephemeris lookups!
        for i in 0..8 {
            let pos = kepler_position(&planets[i], time);
            sources[i + 1] = (i + 1, pos, planets[i].gm);
        }

        sources
    }

    /// Compute acceleration from sources
    pub fn compute_acceleration(
        pos: DVec2,
        sources: &[(DVec2, f64); GRAVITY_SOURCE_COUNT],
    ) -> DVec2 {
        let mut acc = DVec2::ZERO;
        for &(body_pos, gm) in sources {
            let delta = body_pos - pos;
            let r_sq = delta.length_squared();
            if r_sq > 1.0 {
                let r = r_sq.sqrt();
                acc += delta * (gm / (r_sq * r));
            }
        }
        acc
    }

    /// Find dominant body (which gravity source dominates)
    pub fn find_dominant_body(
        pos: DVec2,
        sources: &[(usize, DVec2, f64); GRAVITY_SOURCE_COUNT],
    ) -> Option<usize> {
        let mut max_acc = 0.0;
        let mut dominant = 0; // Sun

        for &(id, body_pos, gm) in sources {
            let delta = body_pos - pos;
            let r_sq = delta.length_squared();
            if r_sq < 1.0 {
                return Some(id);
            }
            let acc_mag = gm / r_sq;
            if acc_mag > max_acc {
                max_acc = acc_mag;
                dominant = id;
            }
        }

        if dominant == 0 { None } else { Some(dominant) }
    }

    /// Check collision - ANOTHER set of ephemeris lookups!
    pub fn check_collision(pos: DVec2, time: f64, planets: &[PlanetData; 8]) -> Option<usize> {
        // Check Sun
        const SUN_RADIUS: f64 = 6.96e8;
        if pos.length() < SUN_RADIUS * 2.0 {
            return Some(0);
        }

        // Check planets - DUPLICATE ephemeris lookups!
        for i in 0..8 {
            let planet_pos = kepler_position(&planets[i], time);
            let collision_radius = planets[i].radius * COLLISION_MULTIPLIER;
            if (pos - planet_pos).length() < collision_radius {
                return Some(i + 1);
            }
        }

        None
    }

    /// One integration step with CURRENT approach (3x ephemeris lookups)
    pub fn prediction_step(
        pos: DVec2,
        vel: DVec2,
        acc: DVec2,
        dt: f64,
        time: f64,
        planets: &[PlanetData; 8],
    ) -> (DVec2, DVec2, DVec2, Option<usize>, Option<usize>) {
        // Velocity Verlet position update
        let pos_new = pos + vel * dt + acc * (0.5 * dt * dt);

        // First lookup: for acceleration
        let sources = get_gravity_sources(planets, time + dt);
        let acc_new = compute_acceleration(pos_new, &sources);

        // Velocity update
        let vel_new = vel + (acc + acc_new) * (0.5 * dt);

        // Second lookup: for dominant body
        let sources_with_id = get_gravity_sources_with_id(planets, time + dt);
        let dominant = find_dominant_body(pos_new, &sources_with_id);

        // Third lookup: for collision
        let collision = check_collision(pos_new, time + dt, planets);

        (pos_new, vel_new, acc_new, dominant, collision)
    }
}

/// OPTIMIZED APPROACH: Single unified ephemeris lookup
mod optimized {
    use super::*;

    /// Get ALL gravity source data in ONE pass
    pub fn get_gravity_sources_full(
        planets: &[PlanetData; 8],
        time: f64,
    ) -> [GravitySourceFull; GRAVITY_SOURCE_COUNT] {
        const SUN_RADIUS: f64 = 6.96e8;
        let mut sources = [GravitySourceFull {
            id: 0,
            pos: DVec2::ZERO,
            gm: 0.0,
            collision_radius: 0.0,
        }; GRAVITY_SOURCE_COUNT];

        // Sun
        sources[0] = GravitySourceFull {
            id: 0,
            pos: DVec2::ZERO,
            gm: GM_SUN,
            collision_radius: SUN_RADIUS * 2.0,
        };

        // Planets - ONE lookup each
        for i in 0..8 {
            sources[i + 1] = GravitySourceFull {
                id: i + 1,
                pos: kepler_position(&planets[i], time),
                gm: planets[i].gm,
                collision_radius: planets[i].radius * COLLISION_MULTIPLIER,
            };
        }

        sources
    }

    /// Combined acceleration + dominant body + collision check from unified sources
    pub fn compute_all_from_sources(
        pos: DVec2,
        sources: &[GravitySourceFull; GRAVITY_SOURCE_COUNT],
    ) -> (DVec2, Option<usize>, Option<usize>) {
        let mut acc = DVec2::ZERO;
        let mut max_acc_mag = 0.0;
        let mut dominant = 0usize;
        let mut collision = None;

        for source in sources {
            let delta = source.pos - pos;
            let r_sq = delta.length_squared();

            // Collision check
            let dist = r_sq.sqrt();
            if dist < source.collision_radius {
                collision = Some(source.id);
            }

            // Gravity and dominant body
            if r_sq > 1.0 {
                let factor = source.gm / (r_sq * dist);
                acc += delta * factor;

                let acc_mag = source.gm / r_sq;
                if acc_mag > max_acc_mag {
                    max_acc_mag = acc_mag;
                    dominant = source.id;
                }
            }
        }

        let dominant_result = if dominant == 0 { None } else { Some(dominant) };
        (acc, dominant_result, collision)
    }

    /// One integration step with OPTIMIZED approach (1x ephemeris lookup)
    pub fn prediction_step(
        pos: DVec2,
        vel: DVec2,
        acc: DVec2,
        dt: f64,
        time: f64,
        planets: &[PlanetData; 8],
    ) -> (DVec2, DVec2, DVec2, Option<usize>, Option<usize>) {
        // Velocity Verlet position update
        let pos_new = pos + vel * dt + acc * (0.5 * dt * dt);

        // SINGLE unified lookup
        let sources = get_gravity_sources_full(planets, time + dt);
        let (acc_new, dominant, collision) = compute_all_from_sources(pos_new, &sources);

        // Velocity update
        let vel_new = vel + (acc + acc_new) * (0.5 * dt);

        (pos_new, vel_new, acc_new, dominant, collision)
    }
}

/// Trajectory point storage (what gets saved)
#[derive(Clone, Copy)]
struct TrajectoryPoint {
    #[allow(dead_code)]
    pos: DVec2,
    #[allow(dead_code)]
    time: f64,
    #[allow(dead_code)]
    dominant_body: Option<usize>,
}

/// Run a full trajectory prediction and return (elapsed_time, step_count, points)
fn run_prediction(
    initial_pos: DVec2,
    initial_vel: DVec2,
    max_time: f64,
    max_steps: usize,
    use_optimized: bool,
    planets: &[PlanetData; 8],
) -> (std::time::Duration, usize, Vec<TrajectoryPoint>) {
    let start = Instant::now();

    let mut pos = initial_pos;
    let mut vel = initial_vel;
    let mut sim_t = 0.0;
    let mut dt = 3600.0; // 1 hour initial timestep
    let mut points = Vec::with_capacity(max_steps / 10);

    // Initialize acceleration
    let sources = optimized::get_gravity_sources_full(planets, 0.0);
    let (mut acc, _, _) = optimized::compute_all_from_sources(pos, &sources);

    let mut step = 0;
    let point_interval = 10;

    while step < max_steps && sim_t < max_time {
        let (pos_new, vel_new, acc_new, dominant, collision) = if use_optimized {
            optimized::prediction_step(pos, vel, acc, dt, sim_t, planets)
        } else {
            current::prediction_step(pos, vel, acc, dt, sim_t, planets)
        };

        pos = pos_new;
        vel = vel_new;
        acc = acc_new;
        sim_t += dt;
        step += 1;

        // Store point at interval
        if step % point_interval == 0 {
            points.push(TrajectoryPoint {
                pos,
                time: sim_t,
                dominant_body: dominant,
            });
        }

        // Simple adaptive timestep (based on velocity)
        let v = vel.length();
        dt = (1e9 / v).clamp(60.0, 86400.0); // 1 min to 1 day

        // Check termination conditions
        if collision.is_some() {
            break;
        }
        if pos.length() > 100.0 * AU || pos.length() < 1e9 {
            break;
        }
    }

    (start.elapsed(), step, points)
}

/// Benchmark individual operations
fn benchmark_operations(planets: &[PlanetData; 8]) {
    println!("\n=== Per-Operation Cost Analysis ===\n");

    let test_pos = DVec2::new(1.5 * AU, 0.2 * AU);
    let time = 365.25 * DAY_SECONDS; // 1 year
    const ITERATIONS: usize = 100_000;

    // Benchmark: Single ephemeris lookup
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let t = time + (i as f64) * 0.001;
        let _ = kepler_position(&planets[2], t); // Earth
    }
    let kepler_time = start.elapsed();
    println!(
        "Single Kepler lookup:    {:>8.2} ns/call",
        kepler_time.as_nanos() as f64 / ITERATIONS as f64
    );

    // Benchmark: Current get_gravity_sources (8 Kepler lookups)
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let t = time + (i as f64) * 0.001;
        let _ = current::get_gravity_sources(planets, t);
    }
    let sources_time = start.elapsed();
    println!(
        "get_gravity_sources:     {:>8.2} ns/call ({} Kepler lookups)",
        sources_time.as_nanos() as f64 / ITERATIONS as f64,
        8
    );

    // Benchmark: Current get_gravity_sources_with_id (8 MORE Kepler lookups)
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let t = time + (i as f64) * 0.001;
        let _ = current::get_gravity_sources_with_id(planets, t);
    }
    let sources_id_time = start.elapsed();
    println!(
        "get_gravity_sources_id:  {:>8.2} ns/call ({} Kepler lookups)",
        sources_id_time.as_nanos() as f64 / ITERATIONS as f64,
        8
    );

    // Benchmark: Current check_collision (8 MORE Kepler lookups)
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let t = time + (i as f64) * 0.001;
        let _ = current::check_collision(test_pos, t, planets);
    }
    let collision_time = start.elapsed();
    println!(
        "check_collision:         {:>8.2} ns/call ({} Kepler lookups)",
        collision_time.as_nanos() as f64 / ITERATIONS as f64,
        8
    );

    // Benchmark: Optimized get_gravity_sources_full (8 Kepler lookups TOTAL)
    let start = Instant::now();
    for i in 0..ITERATIONS {
        let t = time + (i as f64) * 0.001;
        let _ = optimized::get_gravity_sources_full(planets, t);
    }
    let full_time = start.elapsed();
    println!(
        "get_gravity_sources_full:{:>8.2} ns/call ({} Kepler lookups)",
        full_time.as_nanos() as f64 / ITERATIONS as f64,
        8
    );

    // Benchmark: Gravity computation from sources
    let sources = current::get_gravity_sources(planets, time);
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = current::compute_acceleration(test_pos, &sources);
    }
    let gravity_time = start.elapsed();
    println!(
        "compute_acceleration:    {:>8.2} ns/call",
        gravity_time.as_nanos() as f64 / ITERATIONS as f64
    );

    // Benchmark: Optimized combined computation
    let sources_full = optimized::get_gravity_sources_full(planets, time);
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = optimized::compute_all_from_sources(test_pos, &sources_full);
    }
    let combined_time = start.elapsed();
    println!(
        "compute_all_from_sources:{:>8.2} ns/call",
        combined_time.as_nanos() as f64 / ITERATIONS as f64
    );

    // Summary
    println!("\n--- Per-Step Ephemeris Cost ---");
    let current_ephemeris =
        sources_time.as_nanos() + sources_id_time.as_nanos() + collision_time.as_nanos();
    let optimized_ephemeris = full_time.as_nanos();
    println!(
        "CURRENT:   {:>8} ns/step (24 Kepler lookups)",
        current_ephemeris / ITERATIONS as u128
    );
    println!(
        "OPTIMIZED: {:>8} ns/step (8 Kepler lookups)",
        optimized_ephemeris / ITERATIONS as u128
    );
    println!(
        "Expected speedup: {:.1}x for ephemeris queries",
        current_ephemeris as f64 / optimized_ephemeris as f64
    );
}

/// Benchmark full trajectory predictions for various time spans
fn benchmark_trajectories(planets: &[PlanetData; 8]) {
    println!("\n=== Full Trajectory Prediction Benchmark ===\n");

    // Initial conditions: asteroid at 1.1 AU with slightly eccentric orbit
    let initial_pos = DVec2::new(1.1 * AU, 0.0);
    let v_circular = (GM_SUN / (1.1 * AU)).sqrt();
    let initial_vel = DVec2::new(0.0, v_circular * 0.95); // Slightly slower = eccentric

    let time_spans = [
        (1.0, "1 year"),
        (5.0, "5 years"),
        (12.0, "12 years"),
        (20.0, "20 years"),
    ];
    let max_steps = 200_000; // Enough for 20 years

    println!("Time Span  | Max Steps | Current           | Optimized         | Speedup");
    println!("-----------|-----------|-------------------|-------------------|--------");

    for (years, label) in time_spans {
        let max_time = years * YEAR_SECONDS;

        // Warmup
        for _ in 0..WARMUP_ITERATIONS {
            let _ = run_prediction(
                initial_pos,
                initial_vel,
                max_time,
                max_steps,
                false,
                planets,
            );
            let _ = run_prediction(initial_pos, initial_vel, max_time, max_steps, true, planets);
        }

        // Benchmark current
        let mut current_total = std::time::Duration::ZERO;
        let mut current_steps = 0;
        for _ in 0..BENCHMARK_ITERATIONS {
            let (elapsed, steps, _) = run_prediction(
                initial_pos,
                initial_vel,
                max_time,
                max_steps,
                false,
                planets,
            );
            current_total += elapsed;
            current_steps = steps;
        }
        let current_avg = current_total / BENCHMARK_ITERATIONS as u32;

        // Benchmark optimized
        let mut opt_total = std::time::Duration::ZERO;
        let mut opt_steps = 0;
        for _ in 0..BENCHMARK_ITERATIONS {
            let (elapsed, steps, _) =
                run_prediction(initial_pos, initial_vel, max_time, max_steps, true, planets);
            opt_total += elapsed;
            opt_steps = steps;
        }
        let opt_avg = opt_total / BENCHMARK_ITERATIONS as u32;

        let speedup = current_avg.as_secs_f64() / opt_avg.as_secs_f64();

        println!(
            "{:<10} | {:>9} | {:>8.2}ms {:>6} | {:>8.2}ms {:>6} | {:.2}x",
            label,
            current_steps.min(opt_steps),
            current_avg.as_secs_f64() * 1000.0,
            format!("({} st)", current_steps),
            opt_avg.as_secs_f64() * 1000.0,
            format!("({} st)", opt_steps),
            speedup
        );
    }
}

/// Memory usage analysis
fn analyze_memory() {
    println!("\n=== Memory Usage Analysis ===\n");

    let point_size = std::mem::size_of::<TrajectoryPoint>();
    println!("TrajectoryPoint size: {} bytes", point_size);

    for steps in [10_000, 50_000, 100_000, 200_000] {
        let points_stored = steps / 10; // Store every 10th point
        let memory_mb = (points_stored * point_size) as f64 / (1024.0 * 1024.0);
        println!(
            "{:>7} steps -> {:>6} points -> {:.2} MB",
            steps, points_stored, memory_mb
        );
    }
}

fn main() {
    println!("=== Trajectory Prediction Benchmark ===");
    println!("This benchmark measures the performance impact of redundant ephemeris lookups");
    println!("and establishes baselines for optimization work.\n");

    let planets = get_planet_data();

    // Individual operation costs
    benchmark_operations(&planets);

    // Full trajectory predictions
    benchmark_trajectories(&planets);

    // Memory analysis
    analyze_memory();

    println!("\n=== Summary ===");
    println!("The CURRENT approach performs 24 Kepler lookups per integration step:");
    println!("  - 8 for get_gravity_sources()");
    println!("  - 8 for get_gravity_sources_with_id()");
    println!("  - 8 for check_collision()");
    println!("\nThe OPTIMIZED approach performs only 8 lookups by fetching all data once.");
    println!("Expected improvement: ~3x reduction in ephemeris query time per step.");
}
