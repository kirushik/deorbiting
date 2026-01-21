//! Integration test for ion beam continuous deflection.
//!
//! This example simulates an asteroid with ion beam deflection and verifies:
//! - Thrust is properly applied over time
//! - Delta-v accumulates correctly
//! - Trajectory changes as expected
//!
//! Run with: cargo run --example test_ion_beam_deflection

use bevy::math::DVec2;

/// Physical constants
const AU_TO_METERS: f64 = 1.495978707e11;
const SECONDS_PER_DAY: f64 = 86400.0;
const G0: f64 = 9.80665;

/// Sun's standard gravitational parameter (GM) - m³/s²
const GM_SUN: f64 = 1.32712440018e20;

/// Ion beam deflector parameters
struct IonBeamDeflector {
    /// Thrust force in Newtons
    thrust_n: f64,
    /// Fuel mass in kg
    fuel_mass_kg: f64,
    /// Specific impulse in seconds
    specific_impulse: f64,
    /// Fuel consumed so far in kg
    fuel_consumed: f64,
    /// Accumulated delta-v in m/s
    accumulated_delta_v: f64,
}

impl IonBeamDeflector {
    fn new(thrust_n: f64, fuel_mass_kg: f64, specific_impulse: f64) -> Self {
        Self {
            thrust_n,
            fuel_mass_kg,
            specific_impulse,
            fuel_consumed: 0.0,
            accumulated_delta_v: 0.0,
        }
    }

    /// Check if fuel remains
    fn has_fuel(&self) -> bool {
        self.fuel_consumed < self.fuel_mass_kg
    }

    /// Calculate fuel consumption rate (kg/s)
    fn fuel_rate(&self) -> f64 {
        self.thrust_n / (self.specific_impulse * G0)
    }

    /// Calculate acceleration on asteroid (m/s²)
    fn acceleration(&self, asteroid_mass: f64) -> f64 {
        if asteroid_mass <= 0.0 || !self.has_fuel() {
            return 0.0;
        }
        self.thrust_n / asteroid_mass
    }

    /// Update fuel consumption and delta-v for a time step
    fn update(&mut self, dt: f64, asteroid_mass: f64) {
        if !self.has_fuel() {
            return;
        }

        let fuel_used = self.fuel_rate() * dt;
        let actual_fuel_used = fuel_used.min(self.fuel_mass_kg - self.fuel_consumed);
        self.fuel_consumed += actual_fuel_used;

        // Delta-v = a × dt = (F/m) × dt
        let dv = self.acceleration(asteroid_mass) * dt;
        self.accumulated_delta_v += dv;
    }
}

/// Compute gravitational acceleration at a position from Sun only.
fn compute_gravity(pos: DVec2) -> DVec2 {
    let r_sq = pos.length_squared();
    if r_sq < 1.0 {
        return DVec2::ZERO;
    }
    let r = r_sq.sqrt();
    -pos * (GM_SUN / (r_sq * r))
}

/// Velocity Verlet integrator with optional thrust
struct IntegratorState {
    pos: DVec2,
    vel: DVec2,
    acc: DVec2,
    dt: f64,
}

impl IntegratorState {
    fn new(pos: DVec2, vel: DVec2, dt: f64) -> Self {
        let acc = compute_gravity(pos);
        Self { pos, vel, acc, dt }
    }

    fn step(&mut self, thrust_acc: DVec2) {
        // Position update: x' = x + v×dt + 0.5×a×dt²
        self.pos = self.pos + self.vel * self.dt + self.acc * (0.5 * self.dt * self.dt);

        // New acceleration = gravity + thrust
        let acc_new = compute_gravity(self.pos) + thrust_acc;

        // Velocity update: v' = v + 0.5×(a + a')×dt
        self.vel = self.vel + (self.acc + acc_new) * (0.5 * self.dt);
        self.acc = acc_new;
    }
}

/// Simulate asteroid with and without deflection
fn simulate_deflection(
    initial_pos: DVec2,
    initial_vel: DVec2,
    asteroid_mass: f64,
    deflector: &mut IonBeamDeflector,
    simulation_days: f64,
    dt: f64,
) -> (DVec2, DVec2, DVec2, DVec2) {
    // Simulate WITH deflection
    let mut with_deflection = IntegratorState::new(initial_pos, initial_vel, dt);

    // Simulate WITHOUT deflection (baseline)
    let mut without_deflection = IntegratorState::new(initial_pos, initial_vel, dt);

    let total_time = simulation_days * SECONDS_PER_DAY;
    let mut sim_t = 0.0;

    while sim_t < total_time {
        // Calculate thrust direction (retrograde - opposing velocity)
        let thrust_dir = if with_deflection.vel.length() > 0.0 {
            -with_deflection.vel.normalize()
        } else {
            DVec2::ZERO
        };

        // Calculate thrust acceleration if fuel available
        let thrust_acc = if deflector.has_fuel() {
            thrust_dir * deflector.acceleration(asteroid_mass)
        } else {
            DVec2::ZERO
        };

        // Step both integrators
        with_deflection.step(thrust_acc);
        without_deflection.step(DVec2::ZERO);

        // Update deflector state
        deflector.update(dt, asteroid_mass);

        sim_t += dt;
    }

    (
        with_deflection.pos,
        with_deflection.vel,
        without_deflection.pos,
        without_deflection.vel,
    )
}

fn main() {
    println!("=== Ion Beam Deflection Integration Test ===\n");

    // Test 1: Basic ion beam deflection
    test_basic_deflection();

    // Test 2: Delta-v accumulation matches physics
    test_delta_v_accumulation();

    // Test 3: Fuel depletion works correctly
    test_fuel_depletion();

    // Test 4: Trajectory deviation over time
    test_trajectory_deviation();

    println!("\n=== All ion beam deflection tests passed! ===");
}

fn test_basic_deflection() {
    println!("Test 1: Basic ion beam deflection effect...");

    // Asteroid at 1 AU, circular orbit
    let distance = 1.0 * AU_TO_METERS;
    let initial_pos = DVec2::new(distance, 0.0);
    let v_circular = (GM_SUN / distance).sqrt();
    let initial_vel = DVec2::new(0.0, v_circular);

    // 10 billion kg asteroid (small NEA)
    let asteroid_mass = 1e10;

    // Ion beam: 100 mN thrust, 500 kg fuel, Isp 3000 s
    let mut deflector = IonBeamDeflector::new(0.1, 500.0, 3000.0);

    // Simulate for 30 days
    let dt = 3600.0; // 1 hour
    let (pos_deflected, vel_deflected, pos_undeflected, vel_undeflected) = simulate_deflection(
        initial_pos,
        initial_vel,
        asteroid_mass,
        &mut deflector,
        30.0,
        dt,
    );

    // The deflected asteroid should have lower velocity (retrograde thrust)
    let speed_deflected = vel_deflected.length();
    let speed_undeflected = vel_undeflected.length();

    println!("  Initial velocity: {:.6} m/s", v_circular);
    println!("  After 30 days:");
    println!("    Undeflected speed: {:.6} m/s", speed_undeflected);
    println!("    Deflected speed: {:.6} m/s", speed_deflected);
    println!(
        "    Speed difference: {:.6} m/s",
        speed_undeflected - speed_deflected
    );
    println!(
        "    Accumulated Δv: {:.6} m/s",
        deflector.accumulated_delta_v
    );

    assert!(
        speed_deflected < speed_undeflected,
        "Deflected asteroid should be slower (retrograde thrust)"
    );
    assert!(
        deflector.accumulated_delta_v > 0.0,
        "Should have accumulated some delta-v"
    );

    println!("  PASSED\n");
}

fn test_delta_v_accumulation() {
    println!("Test 2: Delta-v accumulation matches physics...");

    let asteroid_mass = 1e10; // 10 billion kg
    let thrust = 0.1; // 100 mN
    let fuel_mass = 100.0; // 100 kg (enough for ~10 days)
    let isp = 3000.0;

    let mut deflector = IonBeamDeflector::new(thrust, fuel_mass, isp);

    // Expected acceleration: a = F/m = 0.1 / 1e10 = 1e-11 m/s²
    let expected_acc = thrust / asteroid_mass;

    // Simulate for 5 days (should not deplete fuel)
    let dt = 3600.0;
    let simulation_time = 5.0 * SECONDS_PER_DAY;
    let mut sim_t = 0.0;

    while sim_t < simulation_time {
        deflector.update(dt, asteroid_mass);
        sim_t += dt;
    }

    // Expected delta-v: Δv = a × t = 1e-11 × 5 × 86400 = 4.32e-6 m/s
    let expected_delta_v = expected_acc * simulation_time;
    let relative_error =
        (deflector.accumulated_delta_v - expected_delta_v).abs() / expected_delta_v;

    println!("  Expected acceleration: {:.2e} m/s²", expected_acc);
    println!(
        "  Simulation time: {:.0} days",
        simulation_time / SECONDS_PER_DAY
    );
    println!("  Expected Δv: {:.6e} m/s", expected_delta_v);
    println!("  Actual Δv: {:.6e} m/s", deflector.accumulated_delta_v);
    println!("  Relative error: {:.4}%", relative_error * 100.0);

    assert!(
        relative_error < 0.01,
        "Delta-v should match expected within 1%"
    );

    println!("  PASSED\n");
}

fn test_fuel_depletion() {
    println!("Test 3: Fuel depletion behavior...");

    let asteroid_mass = 1e10;
    let thrust = 0.1; // 100 mN
    let fuel_mass = 50.0; // Small fuel amount
    let isp = 3000.0;

    let mut deflector = IonBeamDeflector::new(thrust, fuel_mass, isp);

    // Fuel consumption rate: mdot = F / (Isp × g0) = 0.1 / (3000 × 9.80665) ≈ 3.4e-6 kg/s
    let fuel_rate = thrust / (isp * G0);
    let expected_duration = fuel_mass / fuel_rate; // seconds until fuel depleted

    println!("  Fuel rate: {:.2e} kg/s", fuel_rate);
    println!(
        "  Expected duration: {:.2} days",
        expected_duration / SECONDS_PER_DAY
    );

    // Simulate until well past fuel depletion
    let dt = 3600.0;
    let total_time = expected_duration * 1.5;
    let mut sim_t = 0.0;

    while sim_t < total_time && deflector.has_fuel() {
        deflector.update(dt, asteroid_mass);
        sim_t += dt;
    }

    let depletion_time = sim_t;

    // Continue simulating - delta-v should not increase
    let delta_v_at_depletion = deflector.accumulated_delta_v;

    while sim_t < total_time {
        deflector.update(dt, asteroid_mass);
        sim_t += dt;
    }

    println!(
        "  Actual depletion time: {:.2} days",
        depletion_time / SECONDS_PER_DAY
    );
    println!("  Δv at depletion: {:.6e} m/s", delta_v_at_depletion);
    println!(
        "  Δv after depletion: {:.6e} m/s",
        deflector.accumulated_delta_v
    );
    println!(
        "  Fuel consumed: {:.2} kg (of {:.2} kg)",
        deflector.fuel_consumed, fuel_mass
    );

    assert!(!deflector.has_fuel(), "Fuel should be depleted");
    assert!(
        (deflector.fuel_consumed - fuel_mass).abs() < 0.1,
        "Should have consumed all fuel"
    );
    assert!(
        (deflector.accumulated_delta_v - delta_v_at_depletion).abs() < 1e-12,
        "Delta-v should not increase after fuel depletion"
    );

    println!("  PASSED\n");
}

fn test_trajectory_deviation() {
    println!("Test 4: Trajectory deviation over time...");

    // Asteroid at 1 AU, on collision course with Earth-like position
    let distance = 1.0 * AU_TO_METERS;
    let initial_pos = DVec2::new(distance, 0.0);
    let v_circular = (GM_SUN / distance).sqrt();
    let initial_vel = DVec2::new(0.0, v_circular);

    let asteroid_mass = 1e10;

    // Strong deflector for observable effect
    let mut deflector = IonBeamDeflector::new(1.0, 5000.0, 3000.0); // 1 N thrust

    // Simulate for 90 days
    let dt = 3600.0;
    let (pos_deflected, _, pos_undeflected, _) = simulate_deflection(
        initial_pos,
        initial_vel,
        asteroid_mass,
        &mut deflector,
        90.0,
        dt,
    );

    // Calculate position deviation
    let deviation = (pos_deflected - pos_undeflected).length();
    let deviation_km = deviation / 1000.0;

    println!("  After 90 days of deflection:");
    println!("    Position deviation: {:.2} km", deviation_km);
    println!(
        "    Accumulated Δv: {:.6} m/s",
        deflector.accumulated_delta_v
    );
    println!("    Fuel consumed: {:.2} kg", deflector.fuel_consumed);

    // With 1 N thrust on 1e10 kg asteroid:
    // a = 1e-10 m/s², over 90 days = 7776000 s
    // Δv ≈ 0.78 mm/s
    // Position change ≈ 0.5 × a × t² ≈ 3000 km (rough estimate)

    // With these parameters, deviation is a few km - this is realistic for
    // ion beam deflection over 90 days
    assert!(
        deviation_km > 1.0,
        "Should have measurable position deviation (> 1 km)"
    );
    assert!(
        deviation_km < 1000.0,
        "Deviation should be reasonable (< 1,000 km)"
    );

    println!("  PASSED\n");
}
