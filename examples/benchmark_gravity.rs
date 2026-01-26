//! Benchmark comparing gravity calculation implementations.
//!
//! Compares three approaches:
//! 1. Naive scalar (current implementation using f64)
//! 2. Wide SIMD (using wide crate for f64x4)
//! 3. Particular crate (specialized N-body library)
//!
//! Run with: cargo run --example benchmark_gravity --release
//!
//! The benchmark simulates 1000 trajectory prediction steps, each computing
//! gravitational acceleration from 15 bodies (1 Sun + 8 Planets + 6 Moons).

use std::time::Instant;

use particular::prelude::*;
use wide::f64x4;

/// Number of gravity sources (Sun + Planets + Moons)
const NUM_BODIES: usize = 15;

/// Number of trajectory prediction steps to simulate
const NUM_STEPS: usize = 10000;

/// Number of iterations for timing stability
const NUM_ITERATIONS: usize = 100;

/// Sun's standard gravitational parameter (GM) - m³/s²
const GM_SUN: f64 = 1.32712440018e20;

/// AU in meters
const AU: f64 = 1.495978707e11;

/// Representative GM values for bodies (scaled for numerical stability)
/// These approximate the real solar system bodies
fn get_body_gm_values() -> [f64; NUM_BODIES] {
    [
        GM_SUN,    // Sun
        2.2032e13, // Mercury
        3.2486e14, // Venus
        3.9860e14, // Earth
        4.2828e13, // Mars
        1.2669e17, // Jupiter
        3.7931e16, // Saturn
        5.7940e15, // Uranus
        6.8351e15, // Neptune
        4.9028e12, // Moon
        5.9594e12, // Io
        3.2027e12, // Europa
        9.8878e12, // Ganymede
        7.1793e12, // Callisto
        8.9781e12, // Titan
    ]
}

/// Generate representative body positions at a given time
fn generate_body_positions(time_offset: f64) -> [[f64; 2]; NUM_BODIES] {
    // Approximate orbital radii in AU, converted to meters
    let radii_au = [
        0.0, 0.387, 0.723, 1.0, 1.524, 5.203, 9.537, 19.19, 30.07,
        0.00257, // Moon (relative to Earth, but we add Earth's position)
        0.00282, 0.00449, 0.00716, 0.01258, // Jupiter moons
        0.00817, // Titan
    ];

    // Approximate orbital periods in days
    let periods_days = [
        1.0, 88.0, 225.0, 365.25, 687.0, 4333.0, 10759.0, 30687.0, 60190.0, 27.3, 1.77, 3.55, 7.15,
        16.69, 15.95,
    ];

    let mut positions = [[0.0; 2]; NUM_BODIES];

    for (i, period_days) in periods_days.iter().enumerate().take(NUM_BODIES) {
        let angle = 2.0 * std::f64::consts::PI * time_offset / (period_days * 86400.0);
        let r = radii_au[i] * AU;

        // For moons, add parent position
        let (base_x, base_y) = if (9..=13).contains(&i) {
            // Jupiter moons
            (positions[5][0], positions[5][1])
        } else if i == 14 {
            // Titan (Saturn's moon)
            (positions[6][0], positions[6][1])
        } else if i == 9 {
            // Earth's moon
            (positions[3][0], positions[3][1])
        } else {
            (0.0, 0.0)
        };

        positions[i] = [base_x + r * angle.cos(), base_y + r * angle.sin()];
    }

    positions
}

// ============================================================================
// Implementation 1: Naive scalar (current approach)
// ============================================================================

fn compute_acceleration_naive(
    pos: [f64; 2],
    body_positions: &[[f64; 2]; NUM_BODIES],
    body_gm: &[f64; NUM_BODIES],
) -> [f64; 2] {
    let mut acc = [0.0, 0.0];

    for i in 0..NUM_BODIES {
        let dx = body_positions[i][0] - pos[0];
        let dy = body_positions[i][1] - pos[1];
        let r_sq = dx * dx + dy * dy;

        if r_sq > 1.0 {
            let r = r_sq.sqrt();
            let factor = body_gm[i] / (r_sq * r);
            acc[0] += dx * factor;
            acc[1] += dy * factor;
        }
    }

    acc
}

fn run_naive_benchmark(
    initial_pos: [f64; 2],
    initial_vel: [f64; 2],
    body_gm: &[f64; NUM_BODIES],
    dt: f64,
) -> [f64; 2] {
    let mut pos = initial_pos;
    let mut vel = initial_vel;

    for step in 0..NUM_STEPS {
        let time = step as f64 * dt;
        let body_positions = generate_body_positions(time);

        // Velocity Verlet step
        let acc = compute_acceleration_naive(pos, &body_positions, body_gm);
        pos[0] += vel[0] * dt + 0.5 * acc[0] * dt * dt;
        pos[1] += vel[1] * dt + 0.5 * acc[1] * dt * dt;

        let body_positions_new = generate_body_positions(time + dt);
        let acc_new = compute_acceleration_naive(pos, &body_positions_new, body_gm);
        vel[0] += 0.5 * (acc[0] + acc_new[0]) * dt;
        vel[1] += 0.5 * (acc[1] + acc_new[1]) * dt;
    }

    pos
}

// ============================================================================
// Implementation 2: Wide SIMD (f64x4) - Optimized
// ============================================================================

/// Softening parameter to avoid singularity (squared, in meters²)
const SOFTENING_SQ: f64 = 1.0;

/// SIMD-friendly body data with precomputed GM (Structure of Arrays)
struct SIMDBodyData {
    x: [f64x4; 4],
    y: [f64x4; 4],
    gm: [f64x4; 4],
}

impl SIMDBodyData {
    fn new(gm: &[f64; NUM_BODIES]) -> Self {
        let mut gm_arr = [[0.0; 4]; 4];
        for i in 0..NUM_BODIES {
            gm_arr[i / 4][i % 4] = gm[i];
        }
        Self {
            x: [f64x4::ZERO; 4],
            y: [f64x4::ZERO; 4],
            gm: [
                f64x4::new(gm_arr[0]),
                f64x4::new(gm_arr[1]),
                f64x4::new(gm_arr[2]),
                f64x4::new(gm_arr[3]),
            ],
        }
    }

    #[inline(always)]
    fn update_positions(&mut self, positions: &[[f64; 2]; NUM_BODIES]) {
        // Convert AoS to SoA format with padding
        for chunk in 0..4 {
            let base = chunk * 4;
            let mut x = [0.0; 4];
            let mut y = [0.0; 4];
            for lane in 0..4 {
                let idx = base + lane;
                if idx < NUM_BODIES {
                    x[lane] = positions[idx][0];
                    y[lane] = positions[idx][1];
                }
            }
            self.x[chunk] = f64x4::new(x);
            self.y[chunk] = f64x4::new(y);
        }
    }
}

#[inline(always)]
fn compute_acceleration_wide(pos: [f64; 2], bodies: &SIMDBodyData) -> [f64; 2] {
    let px = f64x4::splat(pos[0]);
    let py = f64x4::splat(pos[1]);
    let eps = f64x4::splat(SOFTENING_SQ);

    let mut ax = f64x4::ZERO;
    let mut ay = f64x4::ZERO;

    // Process all 16 bodies (4 SIMD registers × 4 lanes)
    for i in 0..4 {
        let dx = bodies.x[i] - px;
        let dy = bodies.y[i] - py;
        let r_sq = dx * dx + dy * dy + eps;
        let r = r_sq.sqrt();
        let factor = bodies.gm[i] / (r_sq * r);
        ax += dx * factor;
        ay += dy * factor;
    }

    let ax_arr = ax.to_array();
    let ay_arr = ay.to_array();
    [
        (ax_arr[0] + ax_arr[1]) + (ax_arr[2] + ax_arr[3]),
        (ay_arr[0] + ay_arr[1]) + (ay_arr[2] + ay_arr[3]),
    ]
}

fn run_wide_benchmark(
    initial_pos: [f64; 2],
    initial_vel: [f64; 2],
    body_gm: &[f64; NUM_BODIES],
    dt: f64,
) -> [f64; 2] {
    let mut pos = initial_pos;
    let mut vel = initial_vel;

    // Reusable SIMD body data (avoids allocation per step)
    let mut bodies = SIMDBodyData::new(body_gm);

    for step in 0..NUM_STEPS {
        let time = step as f64 * dt;
        let body_positions = generate_body_positions(time);
        bodies.update_positions(&body_positions);

        let acc = compute_acceleration_wide(pos, &bodies);
        pos[0] += vel[0] * dt + 0.5 * acc[0] * dt * dt;
        pos[1] += vel[1] * dt + 0.5 * acc[1] * dt * dt;

        let body_positions_new = generate_body_positions(time + dt);
        bodies.update_positions(&body_positions_new);
        let acc_new = compute_acceleration_wide(pos, &bodies);
        vel[0] += 0.5 * (acc[0] + acc_new[0]) * dt;
        vel[1] += 0.5 * (acc[1] + acc_new[1]) * dt;
    }

    pos
}

// ============================================================================
// Implementation 3: Particular crate
// ============================================================================

/// Particle type for particular crate
#[derive(Clone, Copy)]
struct Body {
    position: [f64; 2],
    mu: f64, // GM value
}

impl Particle for Body {
    type Array = [f64; 2];

    fn position(&self) -> [f64; 2] {
        self.position
    }

    fn mu(&self) -> f64 {
        self.mu
    }
}

fn run_particular_benchmark(
    initial_pos: [f64; 2],
    initial_vel: [f64; 2],
    body_gm: &[f64; NUM_BODIES],
    dt: f64,
) -> [f64; 2] {
    let mut pos = initial_pos;
    let mut vel = initial_vel;

    // Use sequential SIMD brute-force (less overhead than parallel for small N)
    // L=4 means process 4 particles at a time with SIMD
    let mut compute_method = sequential::BruteForceSIMD::<4>;

    for step in 0..NUM_STEPS {
        let time = step as f64 * dt;
        let body_positions = generate_body_positions(time);

        // Create bodies for this timestep
        let mut bodies: Vec<Body> = (0..NUM_BODIES)
            .map(|i| Body {
                position: body_positions[i],
                mu: body_gm[i],
            })
            .collect();

        // Add the asteroid as a massless particle
        let asteroid = Body {
            position: pos,
            mu: 0.0, // Massless
        };

        // Get acceleration on asteroid from all bodies
        let accelerations: Vec<[f64; 2]> = std::iter::once(asteroid)
            .chain(bodies.iter().copied())
            .accelerations(&mut compute_method)
            .collect();

        let acc = accelerations[0]; // First is the asteroid

        // Velocity Verlet position update
        pos[0] += vel[0] * dt + 0.5 * acc[0] * dt * dt;
        pos[1] += vel[1] * dt + 0.5 * acc[1] * dt * dt;

        // Recompute for new position
        let body_positions_new = generate_body_positions(time + dt);
        bodies = (0..NUM_BODIES)
            .map(|i| Body {
                position: body_positions_new[i],
                mu: body_gm[i],
            })
            .collect();

        let asteroid_new = Body {
            position: pos,
            mu: 0.0,
        };

        let accelerations_new: Vec<[f64; 2]> = std::iter::once(asteroid_new)
            .chain(bodies.iter().copied())
            .accelerations(&mut compute_method)
            .collect();

        let acc_new = accelerations_new[0];

        // Velocity update
        vel[0] += 0.5 * (acc[0] + acc_new[0]) * dt;
        vel[1] += 0.5 * (acc[1] + acc_new[1]) * dt;
    }

    pos
}

fn main() {
    println!("=== Gravity Calculation Benchmark ===\n");
    println!(
        "Simulating {} trajectory steps with {} gravity sources",
        NUM_STEPS, NUM_BODIES
    );
    println!(
        "Running {} iterations for timing stability\n",
        NUM_ITERATIONS
    );

    let body_gm = get_body_gm_values();

    // Initial conditions: asteroid at 1.1 AU with circular orbit velocity
    let initial_pos = [1.1 * AU, 0.0];
    let v_circular = (GM_SUN / (1.1 * AU)).sqrt();
    let initial_vel = [0.0, v_circular];
    let dt = 3600.0; // 1 hour timestep

    // Warm up
    println!("Warming up...");
    let _ = run_naive_benchmark(initial_pos, initial_vel, &body_gm, dt);
    let _ = run_wide_benchmark(initial_pos, initial_vel, &body_gm, dt);
    let _ = run_particular_benchmark(initial_pos, initial_vel, &body_gm, dt);

    // Benchmark naive implementation
    println!("\n1. Naive scalar implementation (current):");
    let start = Instant::now();
    let mut final_pos_naive = [0.0; 2];
    for _ in 0..NUM_ITERATIONS {
        final_pos_naive = run_naive_benchmark(initial_pos, initial_vel, &body_gm, dt);
    }
    let naive_duration = start.elapsed();
    let naive_per_iter = naive_duration / NUM_ITERATIONS as u32;
    println!(
        "   Total: {:?}, Per iteration: {:?}",
        naive_duration, naive_per_iter
    );
    println!(
        "   Final position: ({:.3e}, {:.3e}) m",
        final_pos_naive[0], final_pos_naive[1]
    );

    // Benchmark wide SIMD implementation
    println!("\n2. Wide SIMD implementation (f64x4):");
    let start = Instant::now();
    let mut final_pos_wide = [0.0; 2];
    for _ in 0..NUM_ITERATIONS {
        final_pos_wide = run_wide_benchmark(initial_pos, initial_vel, &body_gm, dt);
    }
    let wide_duration = start.elapsed();
    let wide_per_iter = wide_duration / NUM_ITERATIONS as u32;
    println!(
        "   Total: {:?}, Per iteration: {:?}",
        wide_duration, wide_per_iter
    );
    println!(
        "   Final position: ({:.3e}, {:.3e}) m",
        final_pos_wide[0], final_pos_wide[1]
    );

    // Benchmark particular implementation
    println!("\n3. Particular crate implementation:");
    let start = Instant::now();
    let mut final_pos_particular = [0.0; 2];
    for _ in 0..NUM_ITERATIONS {
        final_pos_particular = run_particular_benchmark(initial_pos, initial_vel, &body_gm, dt);
    }
    let particular_duration = start.elapsed();
    let particular_per_iter = particular_duration / NUM_ITERATIONS as u32;
    println!(
        "   Total: {:?}, Per iteration: {:?}",
        particular_duration, particular_per_iter
    );
    println!(
        "   Final position: ({:.3e}, {:.3e}) m",
        final_pos_particular[0], final_pos_particular[1]
    );

    // Summary
    println!("\n=== Summary (release mode) ===");
    println!(
        "Naive:      {:>10?} per iteration (baseline)",
        naive_per_iter
    );

    let wide_speedup = naive_duration.as_secs_f64() / wide_duration.as_secs_f64();
    println!(
        "Wide SIMD:  {:>10?} per iteration ({:.2}x vs naive)",
        wide_per_iter, wide_speedup
    );

    let particular_speedup = naive_duration.as_secs_f64() / particular_duration.as_secs_f64();
    println!(
        "Particular: {:>10?} per iteration ({:.2}x vs naive)",
        particular_per_iter, particular_speedup
    );

    println!("\n=== Analysis ===");
    if wide_speedup > 1.0 {
        println!(
            "✓ Wide SIMD provides {:.0}% speedup over naive",
            (wide_speedup - 1.0) * 100.0
        );
    } else {
        println!(
            "✗ Wide SIMD is slower than naive (overhead > benefit for {} bodies)",
            NUM_BODIES
        );
    }
    if particular_speedup < 1.0 {
        println!(
            "✗ Particular has high abstraction overhead for only {} bodies",
            NUM_BODIES
        );
        println!("  Consider particular for N > 100 bodies where O(N²) dominates");
    }

    // Verify results match (within floating point tolerance)
    println!("\n=== Verification ===");
    let tolerance = 1e-6; // Relative tolerance

    let diff_wide = ((final_pos_naive[0] - final_pos_wide[0]).powi(2)
        + (final_pos_naive[1] - final_pos_wide[1]).powi(2))
    .sqrt();
    let baseline = (final_pos_naive[0].powi(2) + final_pos_naive[1].powi(2)).sqrt();
    let rel_diff_wide = diff_wide / baseline;

    let diff_particular = ((final_pos_naive[0] - final_pos_particular[0]).powi(2)
        + (final_pos_naive[1] - final_pos_particular[1]).powi(2))
    .sqrt();
    let rel_diff_particular = diff_particular / baseline;

    println!(
        "Wide vs Naive relative difference: {:.2e} {}",
        rel_diff_wide,
        if rel_diff_wide < tolerance {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "Particular vs Naive relative difference: {:.2e} {}",
        rel_diff_particular,
        if rel_diff_particular < tolerance {
            "✓"
        } else {
            "✗"
        }
    );
}
