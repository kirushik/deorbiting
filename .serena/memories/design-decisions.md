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
- Phase 6 (future): Continuous deflection (ion beam, gravity tractor, laser ablation)

## Key Constants

- J2000 Unix timestamp: 946728000
- G (gravitational constant): 6.67430e-11 m³·kg⁻¹·s⁻²
- AU to meters: 1.495978707e11
