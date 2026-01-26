# Design Decisions Summary

## Core Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Precision | f64 physics, f32 render | Orbital accuracy requires f64 for meter-scale solar system |
| Integrator | IAS15 | Adaptive 15th-order, machine-precision energy conservation |
| Time base | `f64` seconds since J2000 (approx, uniform; ignore TT/UTC + leap seconds) | Good-enough game time axis; avoids heavy time libs | Good balance for observing planetary motion |
| Moons | Treat as heliocentric in ephemeris tables (2D), but keep `CelestialBodyId::parent()` for gameplay/organization | Simplest runtime ephemeris usage; avoids mixed frames | Natural hierarchy: moon_pos = planet_pos + local_orbit |
| Distortion | **REMOVED** | Caused trajectory/velocity mismatches; Z-ordering (asteroids z=3.0 > planets z=2.0) suffices for visibility |
| Moons | **Decorative only** | No gravity contribution (9 sources: Sun + 8 planets), no collision detection |
| Camera | Scroll zoom + drag pan | Standard map controls |
| Orbit data | Table-based ephemeris generated from JPL Horizons (vectors) covering ~200y from J2000 | Much less drift than fixed Kepler elements; still lightweight at runtime | Accuracy with real planetary positions |
| Bevy version | 0.15 | Latest stable |
| Collision | Pause + destroy asteroid + notification | Clear feedback; simulation continues after play |
| Collision zones | Planets 50×, Sun 2×, Moons none | Moons are decorative only |
| Proximity timestep cap | Activates within 3× collision radius | Prevents skipping collisions without slowing distant flybys |
| Reset behavior | Full reset: clear all asteroids, respawn initial | Asteroid positions are time-dependent; keeping user-spawned asteroids at reset would be inconsistent | Flexibility with guided experiences |

## Documentation Structure

- `ARCHITECTURE.md` - Split-world pattern, ECS components, coordinate systems
- `PHYSICS.md` - IAS15 integrator, time system, gravity model
- `EPHEMERIS.md` - Kepler solver, J2000 orbital elements, hierarchical orbits
- `ROADMAP.md` - 6-phase implementation plan (Phase 0-5)
- `UI.md` - Time controls, velocity handle, info panel, scenario menu

## Implementation Notes (Updated 2026-01-16)

| Change | Rationale |
|--------|-----------|
| Fixed-size gravity source arrays | Eliminates ~30k heap allocations per trajectory prediction |
| Unified timestep logic | Prediction and physics use same acceleration-based adaptive timestep |
| Improved error estimation | Uses second central difference of acceleration (proper O(dt³) error proxy) |
| Float accumulation fix | Track elapsed time from zero instead of subtracting from remaining |
| GM_SUN constant centralized | Single source of truth in types.rs |
| Collision handling in collision.rs | Clear module separation |
| SIMD Hermite interpolation | f64x4 for computing pos.x, pos.y, vel.x, vel.y in parallel |
| Pre-computed GM cache | GM values computed once at startup, stored in fixed array |
| Batched ephemeris sampling | sample_all_positions() avoids per-body HashMap lookups |
| Position-only sampling | sample_position() skips velocity computation when not needed |

### Ephemeris Performance Architecture

Two code paths in `get_gravity_sources()`:

1. **Primary (Horizons tables)**: Cubic Hermite interpolation with SIMD f64x4 acceleration.
   - Pre-computed GM values avoid repeated `G * mass` multiplication
   - `sample_all_positions()` batches 14 table lookups with better cache locality
   - `sample_position()` skips velocity computation (half the work)

2. **Fallback (Kepler solver)**: Used when outside table time range (~200 years from J2000).
   - sin/cos calls dominate - unavoidable for orbital mechanics
   - Moons require parent position lookup (hierarchical)

### Benchmark Results (15 gravity sources)

| Approach | Time | Notes |
|----------|------|-------|
| Naive scalar | ~396µs | Baseline |
| Wide SIMD (gravity) | ~360µs | 10% faster, diminishing returns on small N |
| Particular crate | ~1500µs | 4x slower, abstraction overhead |

SIMD in *gravity loop* has marginal benefit for 15 bodies. SIMD in *Hermite interpolation* is always beneficial (4 outputs computed in parallel).

GPU acceleration (particular/wgpu) only beneficial for 1000+ bodies due to dispatch overhead.

### Major Design Changes (2026-01-16)

| Change | Before | After | Rationale |
|--------|--------|-------|-----------|
| **Visual distortion** | Asteroid positions pushed away from inflated planets | Asteroids render at true physics positions | Caused trajectory/velocity mismatches; Z-ordering keeps asteroids visible |
| **Gravity sources** | 15 bodies (Sun + planets + moons) | 9 bodies (Sun + 8 planets) | Moons decorative only; simplifies physics |
| **Collision detection** | Sun, planets, and moons | Sun (2×) and planets (50×) only | Moons decorative only |
| **Proximity timestep cap** | 10% safety factor everywhere | 50% factor, only within 3× collision radius | Avoids unnecessary slowdown during distant flybys |

These changes prioritize **trajectory accuracy over visual tidiness** for an intuition-building simulator.

### Performance Issues Fixed (2026-01)

**Once-per-day stutter**: Removed time-based prediction trigger in `track_selection_changes()`. The code was checking if simulation time advanced by 86400 seconds (1 day) and triggering a full trajectory recalculation (up to 50k integration steps). Frame-based updates (every 10 frames) are sufficient for smooth visualization.

**Redundant ephemeris lookups**: Added `compute_acceleration_from_sources()` and `find_dominant_body_from_sources()` to allow caching and reuse of gravity source positions within prediction loops.

## Phase 5 Implementation (2026-01-16)

### Scenarios
Six educational scenarios implemented in `src/scenarios/`:
1. **Earth Collision Course** - Tutorial, ~23-day collision (default)
2. **Apophis Flyby** - Close Earth approach at 30,000 km
3. **Jupiter Slingshot** - Voyager-style gravity assist
4. **Interstellar Visitor** - Oumuamua-style hyperbolic (e>1, ~32 km/s)
5. **Deflection Challenge** - 6-month lead time, starts paused
6. **Sandbox** - Zero velocity, starts paused

Key files:
- `src/scenarios/mod.rs` - Scenario struct, ScenarioPlugin, load system
- `src/scenarios/presets.rs` - 6 static scenario definitions

### Outcome Detection
Implemented in `src/outcome.rs`:
- `TrajectoryOutcome` enum: InProgress, Collision, Escape, StableOrbit
- `OrbitalElements` struct: a, e, energy, angular momentum, period
- `compute_orbital_elements()` - vis-viva equations
- `detect_outcome()` - classify trajectory from prediction results

Integrated into `TrajectoryPath` component in `src/prediction.rs`.

### Interceptor System
Implemented in `src/interceptor/`:
- **payload.rs**: `DeflectionPayload` enum (Kinetic/Nuclear), delta-v calculations
- **mod.rs**: `Interceptor` component, `InterceptorPlugin`, systems

Physics based on real research:
- **Kinetic Impactor**: Δv = β × (m × v_rel) / M_asteroid (DART formula, β ≈ 3.6)
- **Nuclear Standoff**: ~2 cm/s per 100 kt for 3×10^10 kg asteroid (LLNL research)

### UI Components
- `src/ui/scenario_menu.rs` - Modal window, Escape/M key trigger
- `src/ui/outcome_overlay.rs` - Color-coded overlays (red/blue/green)
- `src/ui/interceptor_launch.rs` - Payload configuration, direction, launch

### Keyboard Shortcuts Added
- Escape or M: Open scenario menu
- 1-4: Quick time scale selection (1x, 10x, 100x, 1000x)

### Reset Behavior Change
`handle_reset()` now sends `LoadScenarioEvent` for current scenario instead of hardcoded asteroid spawn.

### Phase 5/6 Split
- Phase 5 (complete): Instant deflection (kinetic, nuclear)
- Phase 6 (complete): Continuous deflection (ion beam, laser ablation, solar sail)

## Phase 6 Implementation (2026-01-19, updated 2026-01-26)

### Continuous Deflection Methods
Three continuous deflection methods implemented in `src/continuous/`:

1. **Ion Beam Shepherd** - Spacecraft near asteroid, ion exhaust pushes it
   - Formula: `acceleration = thrust_N / asteroid_mass_kg`
   - Fuel consumption: `mdot = thrust / (Isp × g0)`
   - Default: 50 kN thrust, 2,000,000 kg fuel capacity

2. **Laser Ablation** - Earth-based DE-STAR system vaporizes asteroid surface
   - Formula: `thrust_N = 115 × (power_kW / 100) × solar_efficiency` (50x gameplay boost)
   - Default: 50 MW power, 6-month operation duration
   - Zero flight time (ground-based system)

3. **Solar Sail** - Reflects sunlight for radiation pressure thrust
   - Solar radiation pressure: ~9.08e-4 N/m² at 1 AU (100x gameplay boost)
   - Formula: `thrust = sail_area × 9.08e-4 × reflectivity / distance_AU²`
   - Default: 10 km² sail area

**Removed:** Gravity Tractor (too slow for gameplay timescales, redundant with ion beam)

### Additional Instant Payloads
- **NuclearSplit** - Armageddon-style asteroid splitting
  - Splits asteroid into two fragments with diverging trajectories
  - Energy: `E = yield_kt × 4.184e12 J` (TNT equivalent)
  - ~1% energy converted to kinetic separation
  - Separation velocity: `v = sqrt(2 × 0.01 × E / M_asteroid)`
  - Fragments move perpendicular to deflection direction

### Key Components
- `ContinuousDeflector` - Component tracking target, payload, state
- `ContinuousDeflectorState` - EnRoute, Operating, FuelDepleted, Complete, Cancelled
- `ContinuousPayload` - IonBeam, LaserAblation, SolarSail variants
- `ThrustDirection` - Retrograde, Prograde, Radial, Custom

### Physics Integration
- `compute_continuous_thrust()` aggregates thrust from all active deflectors
- `physics_step` modified to include continuous thrust in acceleration
- Prediction system also accounts for continuous thrust
- Multiple deflectors can operate on same asteroid simultaneously

### Key Files
- `src/continuous/thrust.rs` - Thrust calculation functions
- `src/continuous/payload.rs` - ContinuousPayload enum
- `src/continuous/mod.rs` - Component, state machine, plugin
- `src/ui/mission_status.rs` - Active missions panel
- `src/render/deflectors.rs` - Visualization (icons, thrust arrows)

### UI Enhancements (2026-01-20)
- **Asteroid Mass Editor** - Collapsible section in info panel
  - Logarithmic slider (10^6 to 10^15 kg)
  - Quick preset buttons (1e9, 1e10, 1e11, 1e12 kg)
  - Real-time mass modification updates physics/predictions

- **Deflection Challenge Rebalancing**
  - Reduced asteroid mass from 5e11 to 5e9 kg
  - Expanded kinetic impactor slider: 100-10000 kg (was 100-2000)
  - Expanded nuclear yield slider: 1-10000 kt (was 1-1000)
  - Makes scenario solvable with various methods

## Phase 7: Deflection Mechanics Overhaul (2026-01-22)

### Problem
Deflection methods were dramatically ineffective at gameplay timescales. With the Deflection Challenge scenario (5e9 kg asteroid, 46-day lead time), methods provided:
- Kinetic (DART-like): 2.4 mm/s delta-v (needed ~80 m/s)
- Nuclear (100 kt): 120 mm/s delta-v (0.15% of needed)

### Solution: Parameter Inflation for Single-Shot Scenarios
Inflated default parameters for gameplay effectiveness. Key insight: raw delta-v improvement doesn't translate 1:1 to closest-approach improvement due to orbital mechanics.

| Method | Old Default | Gameplay Value | Change |
|--------|-------------|----------------|--------|
| **Kinetic mass (DART)** | 560 kg | 50,000 kg | 89× |
| **Kinetic β** | 3.6 | 20.0 | 5.6× |
| **Heavy kinetic mass** | - | 250,000 kg | New |
| **Nuclear ref Δv** | 0.02 m/s | 0.30 m/s | 15× |
| Ion beam thrust | 0.1 N | 10 N | 100× |
| Laser ablation | 100 kW | 10,000 kW | 100× |
| Solar sail area | 10,000 m² | 1,000,000 m² | 100× |

### Scenario Mass Reduction (5×)
Combined with weapon boosts for ~70× total improvement:

| Scenario | Old Mass | New Mass | Nuclear 1000kt Δv |
|----------|----------|----------|-------------------|
| Deflection Challenge | 5e9 kg | 1e9 kg | 90 m/s ✓ |
| Earth Collision | 5e10 kg | 1e10 kg | 9 m/s |
| Apophis Flyby | 2.7e10 kg | 5e9 kg | 18 m/s |
| Interstellar Visitor | 4e9 kg | 8e8 kg | 112 m/s ✓ |
| Jupiter Slingshot | 5e11 kg | 1e11 kg | 0.9 m/s |
| Sandbox | 1e12 kg | 2e11 kg | 0.45 m/s |

**Deflection Challenge** is now solvable with Nuclear 1000kt (perpendicular burn gives 338k km vs 319k threshold).

### Earth Collision Course Timing
Changed from ~23 days (45° ahead) to ~180 days (177° ahead) to allow realistic continuous deflection.

### Lambert Solver Integration
Added `src/lambert.rs` with universal variable formulation for proper orbital mechanics:
- Solves Lambert's problem for any transfer angle (except 180° degenerate case)
- Handles elliptical, parabolic, and hyperbolic transfers
- Generates curved transfer orbit arc for visualization
- Predicts asteroid position at arrival time using Verlet integration

Key components:
- `solve_lambert(r1, r2, tof, mu, prograde)` - Main solver
- `predict_asteroid_at_time()` - Forward prediction helper
- `generate_transfer_arc()` - Kepler propagation for visualization

### Multiple Launch Handling
- Added `id: u32` field to `Interceptor` component
- Each launch assigned unique sequential ID from registry
- Color differentiation via HSL hue shift: 30° per ID (cycles through 12 colors)
- First interceptor uses base color, subsequent launches get shifted hue

### Visual Effects System (Phase 7.5)

**Impact Effects** (`src/render/effects.rs`):
- `ImpactEffect` component with start_time, duration, position, effect_type
- `SpawnImpactEffectEvent` message for triggering effects from interceptor impacts
- Three effect types:
  - **KineticFlash**: White expanding circle with bright core, fades over 0.5s
  - **NuclearExplosion**: Orange/yellow shockwave ring with particle dots, 2s duration
  - **NuclearSplit**: Two separating rings with connecting energy, 3s duration
- Effects scale with yield (logarithmic) for nuclear payloads

**Continuous Method Visualizations** (`src/render/deflectors.rs`):
- **Ion Beam**: Cone of 7 flickering cyan lines opposite to thrust direction, with particle dots and spacecraft body
- **Laser Ablation**: Red/orange beam incoming from Earth direction, ablation plume at impact point
- **Solar Sail**: Diamond shape perpendicular to sun direction, scaled by sail area, with incoming/reflected solar rays

All effects use gizmos (immediate-mode rendering) for simplicity and performance

## Critical Bug Fix: Singularity Threshold Mismatch (2026-01-22)

### Problem
Trajectory prediction diverged from actual physics simulation due to inconsistent singularity thresholds:
- `compute_acceleration_from_sources()` (main physics): `SINGULARITY_THRESHOLD_SQ = 1e6` (1000m)
- `compute_gravity_full()` (prediction): hardcoded `1.0` (1m)

At 100m from Sun, prediction returned 1.3e16 m/s² while physics returned 2.3e-7 m/s² - a 10^23 factor difference!

### Root Cause
When `compute_gravity_full()` was added for performance optimization, it used a different (much smaller) singularity threshold than the established physics function. This caused predicted trajectories to behave completely differently than actual simulation when asteroids passed within 1000m of any body.

### Fix
Changed both `compute_gravity_full()` and `compute_acceleration_from_full_sources()` to use `SINGULARITY_THRESHOLD_SQ` instead of hardcoded `1.0`.

### Test Added
`test_prediction_gravity_matches_physics_gravity` verifies that both gravity computation paths return identical results at all distances

### Interceptor Struct Changes
Added fields:
- `id: u32` - Unique identifier for color differentiation
- `arrival_position: DVec2` - Predicted asteroid position at arrival
- `transfer_arc: Vec<DVec2>` - Pre-computed trajectory points
- `departure_velocity: DVec2` - Lambert solution departure velocity

## Ephemeris Extrapolation Fix (2026-01-22)

### Problem
When simulation time exceeded table coverage (year 2200+), the offset-based extrapolation formula caused catastrophic errors for outer planets:

| Planet | With Offset Extrapolation | With Pure Kepler |
|--------|--------------------------|------------------|
| Mars at year 2222 | 92 AU | 1.5 AU ✓ |
| Jupiter at year 2222 | 2.9 AU | 5.2 AU ✓ |

### Root Cause
The formula `drifted_dp = offset.dp + offset.dv * dt` multiplied velocity offset by time-past-boundary. At 22 years past table end, this produced ~23 AU position offsets.

Even the simpler constant offset (`offset.dp` only) failed because the offset computed at year 2200 doesn't remain valid 22 years later when the planet has moved 2+ orbits.

### Solution
**Removed offset extrapolation entirely.** Past table end, use pure Kepler with no corrections.

Trade-off: Small discontinuity at boundary (< 0.1 AU for inner planets) vs correct orbital shapes for all time past boundary.

### Regression Test
`test_drifting_offset_does_not_distort_orbits` in `src/ephemeris/mod.rs` verifies Mars < Jupiter distance at year 2222.

## Interceptor Speed Fix (2026-01-23)

### Problem
Flight time estimates in the UI were showing hundreds of days for typical scenarios. The `BASE_INTERCEPTOR_SPEED` of 15 km/s (realistic) wasn't inflated for gameplay like other parameters were in Phase 7.

Example at old speed:
- 0.5 AU distance: 58 days
- 2.0 AU distance: 231 days (longer than time to impact in Earth Collision scenario!)

### Solution
Increased `BASE_INTERCEPTOR_SPEED` from 15,000 m/s to 100,000 m/s (~6.7× increase) to match the gameplay inflation of other parameters.

New flight times:
- 0.1 AU: 1.7 days
- 0.5 AU: 8.7 days
- 2.0 AU: 34.6 days

### Tests Added
`test_flight_time_gameplay_scenarios` verifies flight times are reasonable for gameplay:
- Near-Earth (0.1 AU): < 5 days
- Mid-range (0.5 AU): < 15 days
- Far side (2.0 AU): < 50 days

## Deflection Parameter Boost (2026-01-23)

### Problem
Deflection impacts were providing ~2.9 m/s delta-v per impact, insufficient against asteroids traveling at 29 km/s. Multiple launches weren't accumulating enough delta-v to prevent collisions.

### Analysis
Required delta-v for Earth miss (by 2.5× Earth radius) at various intercept distances:
- 0.25 AU (15 days out): 12.4 m/s
- 0.10 AU (6 days out): 30.9 m/s
- 0.05 AU (3 days out): 61.8 m/s

Old defaults against 300m asteroid (3e10 kg):
- Kinetic (50t, β=20): 1.0 m/s
- Nuclear (2 MT): 6.0 m/s

### Solution: Further Parameter Inflation for Multi-Launch Scenarios

| Payload | Old Default | New Default | Δv vs 300m asteroid |
|---------|-------------|-------------|---------------------|
| **Kinetic mass** | 50,000 kg | 200,000 kg | 8.0 m/s |
| **Kinetic β** | 20.0 | 40.0 | (included above) |
| **Nuclear yield** | 2,000 kt (2 MT) | 12,000 kt (12 MT) | 36.0 m/s |
| **Nuclear split yield** | 5,000 kt (5 MT) | 20,000 kt (20 MT) | N/A |

### Multi-Launch Effectiveness (against 300m asteroid)
- 3 kinetic impacts: 24.0 m/s (effective at 0.25 AU / 15 days)
- 1 nuclear: 36.0 m/s (effective at 0.10 AU / 6 days)
- 4 kinetic impacts: 32.0 m/s (effective at 0.10 AU / 6 days)
- 3 kinetic + 2 nuclear: 96.0 m/s (effective at 0.05 AU / 3 days - emergency)

### Test Coverage
`test_deflection_gameplay_effectiveness` verifies 3-5 launches can protect Earth at typical intercept distances (0.1-0.25 AU).

### Default Asteroid Mass
Changed default mass for user-placed asteroids (via + icon in dock) from 1e12 kg to 3e10 kg.
- Matches the "medium asteroid" baseline used for deflection tuning
- Newly placed asteroids are now deflectable with 3-5 launches
- Users can still adjust mass via the context card editor

## Key Constants

- J2000 Unix timestamp: 946728000
- G (gravitational constant): 6.67430e-11 m³·kg⁻¹·s⁻²
- AU to meters: 1.495978707e11
