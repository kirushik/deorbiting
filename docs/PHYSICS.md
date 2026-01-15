# Physics Specification

## 1. The Integrator: IAS15

We use **IAS15** (Integrator with Adaptive Step-size, 15th order), a Gauss-Radau quadrature-based integrator designed for gravitational dynamics.

### Why IAS15?
 - **Adaptive timestep** with automatic error control - no manual tuning needed
 - **15th-order accuracy** (vs 2nd-order Velocity Verlet or 4th-order RK4)
 - **Machine-precision energy conservation** over 10⁹ orbits
 - **Handles close encounters** and high-eccentricity orbits gracefully
 - **Battle-tested**: Used in the REBOUND N-body library for astronomy research

### Reference
Rein & Spiegel (2015) - *"IAS15: A fast, adaptive, high-order integrator for gravitational dynamics, accurate to machine precision over a billion orbits"*
arXiv:1409.4779

### Algorithm Overview
IAS15 uses a 15th-order implicit integrator based on Gauss-Radau quadrature with adaptive step-size control. The key insight is using the difference between predictor and corrector steps to estimate local truncation error.

```rust
// Conceptual structure (actual implementation is more involved)
struct IAS15State {
    pos: DVec2,
    vel: DVec2,
    acc: DVec2,
    // Gauss-Radau coefficients for 7 substeps
    g: [DVec2; 7],
    b: [DVec2; 7],
    // Adaptive timestep
    dt: f64,
    dt_last_done: f64,
}

impl IAS15State {
    fn step(&mut self, acceleration_fn: impl Fn(DVec2, f64) -> DVec2) {
        // 1. Predict positions at 7 Gauss-Radau nodes
        // 2. Compute accelerations at each node
        // 3. Update g coefficients via iteration until convergence
        // 4. Compute b coefficients from g
        // 5. Advance position and velocity
        // 6. Estimate error and adapt timestep
    }
}
```

### Implementation Strategy
Consider porting from REBOUND (C, BSD license) or implementing from the paper. Key components:
 - Gauss-Radau spacing constants (h values)
 - Coefficient matrices for predictor-corrector
 - Adaptive timestep based on error estimate

## 2. Time System

### Simulation Time
 - **Base rate:** 1 simulation-day = 1 real-second at 1x speed
 - **Time scales:** 1x, 10x, 100x, 1000x
 - At 1x: One Earth year passes in ~6 real minutes
 - At 1000x: One Earth year passes in ~0.36 real seconds

### Physics Execution
 - Physics runs in Bevy's `FixedUpdate` schedule
 - IAS15 adapts its internal timestep automatically based on orbital dynamics
 - Multiple IAS15 substeps may occur per frame at high time acceleration

### Time State
```rust
#[derive(Resource)]
struct SimulationTime {
    current: f64,        // Seconds since J2000 epoch
    scale: f64,          // Time multiplier (1.0, 10.0, 100.0, 1000.0)
    paused: bool,
}
```

## 3. Gravity Model

Newtonian point-mass gravity:

$$F = G \frac{m_1 m_2}{r^2}$$

### Constants
 - **G (Gravitational Constant):** 6.67430 × 10⁻¹¹ m³·kg⁻¹·s⁻²
 - Use real physical constants - camera zoom handles visual scale

### Acceleration Calculation
```rust
const G: f64 = 6.67430e-11;  // m³·kg⁻¹·s⁻²

fn compute_acceleration(
    pos: DVec2,
    time: f64,
    ephemeris: &Ephemeris
) -> DVec2 {
    let mut acc = DVec2::ZERO;

    for (body_pos, body_mass) in ephemeris.get_gravity_sources(time) {
        let delta = body_pos - pos;
        let r_squared = delta.length_squared();
        let r = r_squared.sqrt();

        if r > 1.0 {  // Avoid singularity at zero distance
            let magnitude = G * body_mass / r_squared;
            acc += delta.normalize() * magnitude;
        }
    }

    acc
}
```

### Optimization Notes
 - Sun dominates (~99.86% of solar system mass) - compute Sun first
 - For distant asteroids, can approximate inner planets as single mass at barycenter
 - Moons: Include their parent planet's gravity, moon gravity optional for distant objects

## 4. The Prediction Loop

When the user adjusts the velocity handle, we must predict the future trajectory.

### Method
Run the IAS15 integrator on a **cloned state** to compute future positions without affecting the live simulation.

### Critical Requirement
Inside the prediction loop, you **cannot** use current planet positions. You must query `Ephemeris::get_position(body, t_future)` because planets move while the asteroid travels.

### Parameters
```rust
#[derive(Resource)]
struct PredictionSettings {
    max_steps: usize,      // Maximum prediction iterations (default: 1000)
    max_time: f64,         // Maximum prediction horizon in seconds
    min_dt: f64,           // Minimum timestep for prediction
    update_interval: u32,  // Frames between prediction updates (when not dragging)
}
```

### Prediction Behavior
 - **Always visible:** Prediction line shown during play, not just when paused
 - **Real-time updates:** When dragging velocity handle, update every frame
 - **Periodic updates:** When not interacting, update every N frames
 - **Horizon:** Predict until max_time reached, or collision detected, or stable orbit identified

### Algorithm
```rust
fn predict_trajectory(
    initial_state: &BodyState,
    start_time: f64,
    ephemeris: &Ephemeris,
    settings: &PredictionSettings,
) -> TrajectoryPath {
    let mut state = IAS15State::from_body_state(initial_state);
    let mut points = Vec::with_capacity(settings.max_steps);
    let mut t = start_time;

    for _ in 0..settings.max_steps {
        points.push((state.pos, t));

        // Check for collision
        if ephemeris.check_collision(state.pos, t) {
            break;
        }

        // Check time horizon
        if t - start_time > settings.max_time {
            break;
        }

        // Advance one IAS15 step
        state.step(|pos, time| compute_acceleration(pos, time, ephemeris));
        t += state.dt_last_done;
    }

    TrajectoryPath { points }
}
```

## 5. Collision Detection & Close Approach Handling

### The Challenge

At orbital velocities (30-60 km/s), an asteroid can move 200,000+ km in a single hour.
Earth's physical radius is only 6,371 km. With naive fixed timesteps, asteroids
frequently "skip over" planets without detection.

### Solution: Multi-Layer Approach

We use three complementary techniques, informed by research on adaptive N-body
integrators ([Pham & Rein 2024](https://arxiv.org/abs/2401.02849),
[REBOUND documentation](https://rebound.readthedocs.io/en/latest/integrators/)):

#### 1. Danger Zone (Collision Multiplier)

Instead of using physical radii, we detect collision when an asteroid enters
a "danger zone" - a sphere of influence around each body:

```rust
const COLLISION_MULTIPLIER: f64 = 50.0;  // 50x physical radius

// Earth: 6,371 km × 50 = 318,550 km danger zone
// (approximately the Moon's orbital distance)
```

**Rationale**: For a planetary defense simulator, any asteroid passing within
320,000 km of Earth would require intervention. This makes gameplay achievable
while representing realistic threat thresholds.

#### 2. Proximity-Based Timestep Adaptation

Before each integration step, we check the distance to the nearest celestial
body and cap the timestep to ensure we don't skip over the danger zone:

```rust
// Cap timestep based on proximity
let safety_factor = 0.1;  // Move at most 10% of distance per step
let max_dt_proximity = closest_distance / relative_velocity * safety_factor;
ias15.dt = ias15.dt.min(max_dt_proximity);
```

This is a simplified version of the close-encounter handling in professional
codes like REBOUND's IAS15 and Mercury6's hybrid integrator.

#### 3. Visual Feedback (Danger Zone Rings)

Red rings around planets show the collision detection boundary, making it
clear to players where impacts will register.

### Implementation Notes

- The Sun uses a 2x multiplier (already huge)
- Moons use a 10x multiplier (smaller bodies)
- Timestep adaptation activates automatically when approaching any body
- The minimum timestep (60 seconds) prevents excessive slowdown
