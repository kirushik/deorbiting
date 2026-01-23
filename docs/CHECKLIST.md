# Implementation Checklist

A coding-focused, step-by-step checklist for implementing the orbital mechanics simulator. Each item is a discrete, testable task. Check off items as completed.

> **For AI Agents:** Before starting work, read the relevant Serena memories for context on _why_ decisions were made and _how_ to approach implementation. Use `list_memories` and `read_memory` tools.

---

## Phase 0: Foundation ✓

### 0.1 Project Setup
- [x] Update `Cargo.toml` with dependencies:
  ```toml
  [dependencies]
  bevy = "0.15"
  bevy_egui = "0.31"
  ```
- [x] Verify project compiles with `cargo build`
- [x] Create module structure in `src/` (binary-only, no lib.rs):
  - [x] `src/types.rs` (core physics types)
  - [x] `src/ephemeris/mod.rs` (ephemeris module root)
  - [x] `src/ephemeris/kepler.rs` (Kepler solver)
  - [x] `src/ephemeris/data.rs` (orbital elements constants)

### 0.2 Core Types (`src/types.rs`)
- [x] Define `BodyState` component:
  ```rust
  #[derive(Component, Clone, Debug)]
  pub struct BodyState {
      pub pos: DVec2,
      pub vel: DVec2,
      pub mass: f64,
  }
  ```
- [x] Define `SimulationTime` resource:
  ```rust
  #[derive(Resource)]
  pub struct SimulationTime {
      pub current: f64,      // seconds since J2000
      pub scale: f64,        // time multiplier
      pub paused: bool,
  }
  ```
- [x] Define unit conversion constants:
  ```rust
  pub const AU_TO_METERS: f64 = 1.495978707e11;
  pub const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;
  pub const J2000_UNIX: i64 = 946728000;
  pub const SECONDS_PER_DAY: f64 = 86400.0;
  pub const G: f64 = 6.67430e-11;
  ```
- [x] Implement `unix_to_j2000_seconds()` function
- [x] Implement `j2000_seconds_to_date_string()` for display

### 0.3 Kepler Solver (`src/ephemeris/kepler.rs`)
- [x] Define `KeplerOrbit` struct with all orbital elements
- [x] Implement `solve_eccentric_anomaly()` using Newton's method:
  - [x] Initial guess: E = M (with better guess for high e)
  - [x] Iterate until |delta| < 1e-12 or 50 iterations
  - [x] Handle high eccentricity edge cases
- [x] Implement `get_local_position(time: f64) -> DVec2`:
  - [x] Compute mean anomaly M(t)
  - [x] Solve for eccentric anomaly E
  - [x] Compute true anomaly ν
  - [x] Compute radius r
  - [x] Return (x, y) rotated by ω
- [x] Hierarchical orbits handled via `CelestialBodyId::parent()`

### 0.4 Orbital Data (`src/ephemeris/data.rs`)
- [x] Define `CelestialBodyData` struct (id, mass, radius, orbit, visual_scale)
- [x] Create data for planets:
  - [x] Sun (stationary at origin, mass only)
  - [x] Mercury, Venus, Earth, Mars
  - [x] Jupiter, Saturn, Uranus, Neptune
- [x] Create data for moons:
  - [x] Moon (Earth)
  - [x] Io, Europa, Ganymede, Callisto (Jupiter)
  - [x] Titan (Saturn)
- [x] All values in SI units (meters, kg, radians, seconds)

### 0.5 Ephemeris Resource (`src/ephemeris/mod.rs`)
- [x] Define `Ephemeris` resource with entity-to-ID mappings
- [x] Implement `get_position(entity, time) -> DVec2`
- [x] Implement `get_position_by_id(id, time) -> DVec2`
- [x] Implement `get_gravity_sources(time) -> Vec<(DVec2, f64)>`
- [x] Implement `check_collision(pos, time) -> Option<CelestialBodyId>`
- [x] Handle hierarchical orbits (moons query parent position)

### 0.6 Unit Tests (inline in modules)
- [x] Test Kepler solver convergence for e=0 (circular)
- [x] Test Kepler solver convergence for e=0.2 (Mercury-like)
- [x] Test Kepler solver convergence for e=0.9 (high eccentricity)
- [x] Test Earth position at J2000 epoch (should be ~1 AU from Sun)
- [x] Test Moon position relative to Earth
- [x] Test unit conversions (AU↔meters, deg↔rad)
- [x] Verify orbital period: Earth completes orbit in ~365.25 days

**Phase 0 Acceptance:** ✓ All 27 tests pass, `cargo test` succeeds, `cargo build` succeeds.

---

## Phase 1: The Static Universe

### 1.1 Bevy App Setup (`src/main.rs`)
- [x] Create Bevy `App` with default plugins
- [x] Add orthographic camera:
  ```rust
  Camera3dBundle {
      projection: Projection::Orthographic(ortho),
      ..default()
  }
  ```
- [x] Set initial camera position (z=1000 or appropriate)
- [x] Add `Ephemeris` resource to app
- [x] Add `SimulationTime` resource with current date

### 1.2 Camera System (`src/camera.rs`)
- [x] Create `CameraPlugin`
- [x] Implement zoom system:
  - [x] Read mouse scroll wheel input
  - [x] Apply logarithmic zoom (multiply/divide by factor)
  - [x] Clamp to min/max zoom levels
- [x] Implement pan system:
  - [x] Detect middle mouse or left mouse on background
  - [x] Convert screen delta to world delta
  - [x] Update camera position
- [x] Implement focus system:
  - [x] Detect double-click on any position
  - [x] Smoothly animate camera to center on clicked position
- [x] Define zoom constants (MIN_ZOOM, MAX_ZOOM, ZOOM_SPEED)

### 1.3 Celestial Body Spawning (`src/render/bodies.rs`)
- [x] Create `CelestialBodyPlugin`
- [x] Define `CelestialBody` component (radius, visual_scale, name)
- [x] Create `spawn_solar_system()` startup system:
  - [x] Spawn Sun entity with mesh and material
  - [x] Spawn each planet entity
  - [x] Register each entity in `Ephemeris` resource
- [x] Create sphere meshes for each body:
  - [x] Size based on `radius * visual_scale`
  - [x] Color approximating real appearance
- [x] Spawn moons with parent reference

### 1.4 Position Sync System (`src/render/sync.rs`)
- [x] Create system `sync_celestial_positions`:
  - [x] Query all entities with `CelestialBody`
  - [x] Get position from `Ephemeris::get_position(entity, time)`
  - [x] Convert DVec2 (f64) to Vec3 (f32) for Transform
  - [x] Apply appropriate z-layer (2.0 for celestial bodies)
- [x] Add system to `Update` schedule

### 1.5 Visual Polish (`src/render/background.rs`)
- [x] Create starfield background:
  - [x] Spawn many small white dots at z=0
  - [x] Random positions covering viewport
  - [ ] Or: use a textured quad
- [x] Add simple directional light (from Sun direction)
- [x] Create planetary rings for Saturn (flat annulus mesh)
  - [ ] Optional: Uranus and Neptune rings

### 1.6 Time Advancement
- [x] Create `advance_time` system:
  - [x] Read `SimulationTime.scale` and `paused`
  - [x] Add `delta_seconds * scale * SECONDS_PER_DAY` to `current`
- [x] Run in `Update` schedule

### 1.7 Phase 1 Extensions (Visual Polish for "Google Maps" UX)
- [x] Keyboard shortcuts (`src/input.rs`):
  - [x] Space: toggle pause
  - [x] +/-: zoom in/out
  - [x] [/]: decrease/increase time scale
  - [x] R: reset simulation time
- [x] Orbit path ellipses (`src/render/orbits.rs`):
  - [x] Draw dashed elliptical paths for planetary orbits
  - [x] Use Bevy Gizmos for line rendering
  - [x] Configurable visibility and alpha
- [x] Hover highlighting (`src/render/highlight.rs`):
  - [x] Detect mouse hover over celestial bodies
  - [x] Draw cyan highlight ring around hovered body
- [x] Body labels (`src/render/labels.rs`):
  - [x] Render planet/moon names using bevy_egui
  - [x] Position labels in screen space near bodies
  - [x] Fade out when zoomed too far
- [x] Left-click pan on background (in addition to middle mouse)

**Phase 1 Acceptance:** App runs, shows Sun and planets orbiting correctly, camera zoom/pan works, planets at correct positions for current date. Orbit paths visible, hover highlighting works, body labels shown.

---

## Phase 2: Split-World & GUI

### 2.1 bevy_egui Setup
- [x] Add `EguiPlugin` to app
- [x] Create `UiPlugin` module (`src/ui/mod.rs`)

### 2.2 Time Controls Panel (`src/ui/time_controls.rs`)
- [x] Create bottom panel with `egui::TopBottomPanel::bottom`
- [x] Add play/pause button (toggle `SimulationTime.paused`)
- [x] Add time scale buttons (1x, 10x, 100x, 1000x)
- [x] Display current simulation date/time formatted
- [x] Add reset button (store initial time for reset)
- [x] Style with semi-transparent dark background

### 2.3 Visual Distortion - REMOVED
- [x] ~~Implement `apply_visual_distortion()` function~~ → **Removed**
- [x] ~~Create `find_nearest_planet()` helper~~ → **Removed**

> **Decision (2026-01):** Visual distortion was implemented but later removed because:
> - Caused trajectory/velocity arrow mismatches (different coordinate systems)
> - Z-ordering already keeps asteroids visible (z=3.0 > planets z=2.0)
> - Trajectory accuracy is more important than visual tidiness for an intuition-building tool
> - `src/distortion.rs` was deleted; asteroids now render at true physics positions

### 2.4 Info Panel (`src/ui/info_panel.rs`)
- [x] Create right side panel with `egui::SidePanel::right`
- [x] Add collapse/expand button
- [x] Display selected body name
- [x] Display position (X, Y)
- [x] Display velocity magnitude (for asteroids with BodyState)
- [x] Add km/AU unit toggle
- [x] Create `SelectedBody` resource to track selection
- [x] Add asteroid mass editor (collapsible section):
  - [x] Logarithmic slider (10^6 to 10^15 kg)
  - [x] Quick preset buttons (1e9, 1e10, 1e11, 1e12 kg)
  - [x] Real-time mass modification

### 2.5 Body Selection
- [x] Implement mouse picking:
  - [x] Convert screen coords to world coords
  - [x] Find nearest body within click radius
  - [x] Update `SelectedBody` resource
- [x] Add visual highlight for selected body (glow/ring)

**Phase 2 Acceptance:** Time controls work, can pause/play/change speed, info panel shows selected body data, visual distortion ready (no asteroid yet to test).

---

## Phase 3: Asteroid Physics ✓

### 3.1 Asteroid Entity (`src/asteroid.rs`)
- [x] Define `Asteroid` marker component
- [x] Create `spawn_asteroid()` function:
  - [x] Create entity with `BodyState`, `Asteroid`, mesh
  - [x] Initial position near Earth (e.g., 1.01 AU)
  - [x] Initial velocity for roughly circular orbit
- [x] Add to startup or scenario loading

### 3.2 Integrator (`src/physics/integrator.rs`)
- [x] Define integrator state struct (using Velocity Verlet for now, IAS15 future)
- [x] Implement adaptive timestep control
- [x] Implement `step()` method with symplectic integration
- [x] Implement `from_body_state()` constructor
- [x] Add error tolerance constant

> Note: Currently using Velocity Verlet (2nd order symplectic) instead of IAS15.
> Verlet provides excellent energy conservation for orbital mechanics.
> IAS15 can be implemented later for higher precision if needed.

### 3.3 Gravity Calculation (`src/physics/gravity.rs`)
- [x] Implement `compute_acceleration(pos, time, ephemeris) -> DVec2`
- [x] Sum gravitational acceleration from 9 bodies: Sun + 8 planets
- [x] Handle singularity (skip if r < threshold)

> **Note:** Moons are decorative only - they do not contribute to gravity calculations.
> This simplifies physics while remaining educationally accurate.

### 3.4 Physics System (`src/physics/mod.rs`)
- [x] Create `PhysicsPlugin`
- [x] Create `physics_step` system:
  - [x] Skip if `SimulationTime.paused`
  - [x] Query asteroids with `BodyState`
  - [x] Run integration step(s) for time delta
  - [x] Update `BodyState` with new pos/vel
- [x] Run in `FixedUpdate` schedule
- [x] Handle time scaling (multiple substeps if needed)

### 3.5 Asteroid Rendering
- [x] Create `sync_asteroid_positions` system in `sync.rs`:
  - [x] Query entities with `Asteroid` and `BodyState`
  - [x] ~~Apply visual distortion~~ → **Removed** (renders at true physics position)
  - [x] Set Transform at z=SPACECRAFT (3.0) - ensures visibility on top of planets
- [x] Create gray sphere mesh for asteroid

### 3.6 Collision Detection (`src/collision.rs`)
- [x] Create `check_collisions` system:
  - [x] Query asteroids
  - [x] Call `Ephemeris::check_collision()` - Sun (2×) and planets (50×) only
  - [x] If collision: set `SimulationTime.paused = true`
  - [x] Store collision event for UI
- [x] Define `CollisionEvent` event type
- [x] Create `CollisionPlugin`

> **Note:** Moons have no collision detection - asteroids pass through them.

### 3.7 Physics Tests
- [x] Test energy conservation over 100 orbits (<1e-4 relative error)
- [x] Test circular orbit stability (asteroid around Sun)
- [x] Test Earth-like orbit period matches ~365 days (<1% error)
- [x] Integration test example (`examples/test_asteroid_orbit.rs`)

**Phase 3 Acceptance:** ✓ Asteroid spawns, physics integration works, energy conserved (<1e-12 for elliptical, machine precision for circular), collision detection ready, all 44 tests pass.

---

## Phase 4: Trajectory Prediction ✓

### 4.1 Prediction Resource (`src/prediction.rs`)
- [x] Define `PredictionSettings` resource:
  ```rust
  #[derive(Resource)]
  pub struct PredictionSettings {
      pub max_steps: usize,
      pub max_time: f64,
      pub update_interval: u32,
  }
  ```
- [x] Define `TrajectoryPath` component:
  ```rust
  #[derive(Component)]
  pub struct TrajectoryPath {
      pub points: Vec<(DVec2, f64)>,
  }
  ```

### 4.2 Prediction System
- [x] Create `predict_trajectory` system:
  - [x] Clone asteroid's `BodyState`
  - [x] Create new `IAS15State` from clone
  - [x] Run simulation loop for max_steps
  - [x] Store (position, time) at each step
  - [x] Stop early on collision or max_time
  - [x] Update `TrajectoryPath` component

### 4.3 Trajectory Rendering (`src/prediction.rs`)
- [x] Create `draw_trajectory` system using Bevy Gizmos:
  - [x] Query entities with `TrajectoryPath`
  - [x] Draw line segments at true physics positions (no distortion)
  - [x] Color segments based on gravitationally dominant body
  - [x] Use z=1.0 (trajectory layer)
- [x] Add color gradient (fade with time/distance)

### 4.4 Velocity Handle (`src/ui/velocity_handle.rs`)
- [x] Create `VelocityHandle` component
- [x] Spawn arrow mesh when asteroid selected:
  - [x] Base at asteroid position
  - [x] Length proportional to velocity (log scale)
  - [x] Direction = velocity direction
- [x] Implement drag interaction:
  - [x] Detect click on arrow tip
  - [x] Track drag delta
  - [x] Update `BodyState.vel` in real-time
  - [x] Trigger prediction recalculation
- [x] Update arrow visual during drag

### 4.5 Prediction Updates
- [x] Recalculate prediction when:
  - [x] Velocity handle dragged
  - [x] Asteroid selected
  - [x] Every N frames (configurable)
- [x] Run prediction in separate system, not blocking render

### 4.6 Prediction Tests
- [x] Integration test example (`examples/test_trajectory_prediction.rs`)

**Phase 4 Acceptance:** ✓ Trajectory line visible and accurate, velocity handle works, dragging updates prediction in real-time. All 55 tests pass.

---

## Phase 5: Scenarios, Deflection & Polish

> **Design Notes (2026-01):** Phase 5 expanded based on research into real asteroid scenarios
> (Apophis, Oumuamua, Voyager gravity assists) and deflection technologies (DART, nuclear standoff).
> Interceptor system added for planetary defense gameplay. Continuous deflection methods
> (ion beam, gravity tractor, laser) deferred to Phase 6.

### 5.1 Scenario System (`src/scenarios/mod.rs`)
- [x] Define `Scenario` struct:
  ```rust
  pub struct Scenario {
      pub id: &'static str,           // Unique identifier
      pub name: &'static str,         // Display name
      pub description: &'static str,  // Brief explanation
      pub asteroid_pos: DVec2,        // Initial position (meters)
      pub asteroid_vel: DVec2,        // Initial velocity (m/s)
      pub start_time: f64,            // J2000 seconds
      pub camera_center: Option<DVec2>, // Auto-position camera
      pub camera_zoom: Option<f32>,   // Auto-zoom level
      pub time_scale: Option<f64>,    // Initial time scale
  }
  ```
- [x] Create `load_scenario()` function:
  - [x] Despawn all existing asteroids
  - [x] Clear collision state and interceptors
  - [x] Reset SimulationTime to scenario.start_time
  - [x] Spawn asteroid with scenario position/velocity
  - [x] Set camera position/zoom if specified
  - [x] Trigger trajectory recalculation
  - [x] Unpause simulation

### 5.2 Preset Scenarios (`src/scenarios/presets.rs`)

Six scenarios designed for educational value (simplified for dramatic effect):

- [x] **Earth Collision Course** (Default/Tutorial):
  - [x] Asteroid 45° ahead of Earth, retrograde orbit → collision ~23 days
  - [x] Camera centered on Earth, 2 AU field of view
  - [x] Time scale 10x
- [x] **Apophis Flyby** (Gravity assist demo):
  - [x] Close Earth approach (0.0002 AU / 30,000 km)
  - [x] Shows orbital period change after encounter
  - [x] Camera centered on Earth, 0.1 AU close-up view
- [x] **Jupiter Slingshot** (Classic gravity assist):
  - [x] Asteroid approaching Jupiter from behind (gains ~10 km/s)
  - [x] Camera centered on Jupiter, 2 AU field of view
  - [x] Time scale 100x to watch flyby
- [x] **Interstellar Visitor** (Oumuamua-style):
  - [x] Hyperbolic trajectory (e > 1.0), ~30 km/s
  - [x] Camera centered on Sun, 10 AU field of view
  - [x] Demonstrates escape trajectories
- [x] **Deflection Challenge** (Planetary defense game):
  - [x] Asteroid on collision course, 6-12 months lead time
  - [x] Goal: Apply minimal delta-v to make asteroid miss Earth
  - [x] Educational: tiny changes early = huge miss distance
- [x] **Sandbox** (Free experimentation):
  - [x] Asteroid near Earth (1.05 AU), zero velocity
  - [x] User experiments with velocity handle
  - [x] Paused initially

### 5.3 Outcome Detection (`src/outcome.rs`)

Three possible trajectory outcomes with distinct visual feedback:

- [x] Define `TrajectoryOutcome` enum:
  ```rust
  pub enum TrajectoryOutcome {
      Collision { event: CollisionEvent },
      Escape { escape_velocity: f64, direction: DVec2 },
      StableOrbit { semi_major_axis: f64, eccentricity: f64, period: f64 },
      InProgress,
  }
  ```
- [x] Implement orbital energy calculation:
  - [x] `E = 0.5*v² - GM/r` (specific orbital energy)
  - [x] E > 0 → hyperbolic (escaping)
  - [x] E < 0 → bound orbit
- [x] Implement eccentricity calculation from state vectors
- [x] Detect outcomes:
  - [x] Collision: existing collision detection
  - [x] Escape: E > 0 and r > 50 AU outbound
  - [x] Stable orbit: E < 0 and trajectory completes without collision

### 5.4 Interceptor System (`src/interceptor/`)

Instant deflection methods (kinetic impactor, nuclear standoff):

- [x] Define `Interceptor` component:
  ```rust
  pub struct Interceptor {
      pub target: Entity,
      pub payload: DeflectionPayload,
      pub launch_time: f64,
      pub arrival_time: f64,
      pub deflection_direction: DVec2,
      pub state: InterceptorState,
  }
  ```
- [x] Define `DeflectionPayload` enum:
  - [x] `Kinetic { mass_kg: f64, beta: f64 }` (100-10000 kg, β=1.0-5.0)
  - [x] `Nuclear { yield_kt: f64 }` (1-10000 kt)
  - [x] `NuclearSplit { yield_kt: f64, split_ratio: f64 }` (Armageddon-style splitting)
- [x] Implement delta-v calculations:
  - [x] Kinetic: `Δv = β × (m × v_rel) / M_asteroid` (DART formula)
  - [x] Nuclear: ~2 cm/s per 100 kt for 300m asteroid (LLNL research)
  - [x] NuclearSplit: Splits asteroid into two fragments with separation velocity
    - [x] Energy: `E = yield_kt × 4.184e12 J` (TNT equivalent)
    - [x] ~1% energy converted to kinetic separation
    - [x] Separation velocity: `v = sqrt(2 × 0.01 × E / M_asteroid)`
    - [x] Fragments move perpendicular to deflection direction (momentum conserved)
- [x] Create `update_interceptors` system:
  - [x] Track in-flight interceptors
  - [x] Apply delta-v on arrival (or spawn fragments for NuclearSplit)
  - [x] Trigger trajectory recalculation
- [x] Create `handle_asteroid_splitting` system:
  - [x] Despawn original asteroid
  - [x] Spawn two fragment asteroids with diverging trajectories
  - [x] Mass distribution based on split_ratio

### 5.5 Interceptor Launch UI (`src/ui/interceptor_launch.rs`)

- [x] Create launch modal window:
  - [x] Payload type selector (Kinetic / Nuclear / NuclearSplit / Continuous methods)
  - [x] Parameter sliders (mass, beta, yield, split_ratio)
  - [x] Direction control widget (retrograde/prograde/radial/custom)
  - [x] Estimated flight time display
  - [x] Estimated delta-v display
  - [x] Launch button
- [x] Add "Launch Interceptor" button to info panel
- [x] NuclearSplit-specific controls:
  - [x] Yield slider (100-10000 kt, logarithmic)
  - [x] Split ratio slider (20%/80% to 80%/20%)

### 5.6 Scenario Menu (`src/ui/scenario_menu.rs`)
- [x] Create modal window with `egui::Window`
- [x] List all 6 scenarios with descriptions
- [x] Radio button selection
- [x] Load and Cancel buttons
- [x] Trigger via menu button, Escape, or M key
- [x] Pause simulation while menu open

### 5.7 Outcome Overlay (`src/ui/outcome_overlay.rs`)

Three distinct overlays with visual differentiation:

- [x] **Collision overlay** (red):
  - [x] "COLLISION PREDICTED" heading with flash
  - [x] Body hit, impact velocity
  - [x] Reset Scenario / New Scenario buttons
- [x] **Escape overlay** (blue/cyan):
  - [x] "ESCAPE TRAJECTORY" heading
  - [x] Escape velocity (v_infinity), direction
  - [x] Trajectory fading to infinity visual
- [x] **Stable orbit overlay** (green):
  - [x] "STABLE ORBIT" heading
  - [x] Orbital parameters (a, e, period, perihelion, aphelion)
  - [x] Congratulations message for Deflection Challenge

### 5.8 Keyboard Shortcuts Update (`src/input.rs`)
- [x] Space: play/pause *(existing)*
- [x] +/-: zoom *(existing)*
- [x] [/]: time scale *(existing)*
- [x] R: reset *(existing)*
- [x] Escape or M: open scenario menu
- [x] 1-4: quick time scale selection

### 5.9 Visual Effects
- [x] Impact flash effect (brief screen overlay that fades)
- [x] Interceptor trajectory line (from Earth to intercept point)
- [x] Outcome color coding (red/blue/green for overlays)

### 5.10 Final Polish
- [x] Performance profiling (target 60 FPS)
- [x] Window resize handling
- [x] Error handling for edge cases
- [x] Code documentation for new modules

**Phase 5 Acceptance:** ✓ All features complete. All 6 scenarios load correctly with auto-camera, outcome detection works (collision/escape/capture), interceptor system deflects asteroids with correct physics, keyboard shortcuts work. UI polish complete: Launch Interceptor button in info panel, Reset/New Scenario buttons in collision overlay, congratulations message for Deflection Challenge, escape trajectory fading visual, performance diagnostics enabled.

---

## Phase 6: Advanced Deflection ✓

> **Note:** Phase 6 implements continuous deflection methods that require
> ongoing spacecraft operation. These apply small forces over extended periods.

### 6.1 Continuous Deflection Methods

- [x] **Ion Beam Shepherd** (`src/continuous/thrust.rs`):
  - [x] Spacecraft hovers near asteroid, ion exhaust pushes it
  - [x] Continuous low thrust (~10-1000 mN for months)
  - [x] Fuel consumption: `mdot = thrust / (Isp × g0)`
  - [x] Formula: `acceleration = thrust_N / asteroid_mass_kg`
- [x] **Gravity Tractor** (`src/continuous/thrust.rs`):
  - [x] Spacecraft mass gravitationally pulls asteroid
  - [x] Formula: `acceleration = G × spacecraft_mass / distance²`
  - [x] Example: 20,000 kg at 200m → ~0.033 N force
- [x] **Laser Ablation** (`src/continuous/thrust.rs`):
  - [x] Vaporize asteroid surface, creating thrust
  - [x] Solar efficiency: `min(1.0, 1/distance_AU²)`
  - [x] Formula: `thrust_N = 2.3 × (power_kW / 100) × solar_efficiency`
- [x] **Solar Sail** (`src/continuous/thrust.rs`):
  - [x] Reflects sunlight to generate thrust via radiation pressure
  - [x] Solar pressure: ~9.08 μN/m² at 1 AU
  - [x] Scales with 1/r² (inverse square law)
  - [x] Formula: `acceleration = (sail_area × 9.08e-6 × reflectivity) / (distance_AU² × asteroid_mass)`

### 6.2 Component & State Machine (`src/continuous/mod.rs`)

- [x] `ContinuousDeflector` component with target entity and payload
- [x] `ContinuousDeflectorState` enum:
  - [x] EnRoute (traveling to asteroid)
  - [x] Operating (active thrust, tracking fuel/delta-v)
  - [x] FuelDepleted (out of propellant)
  - [x] Complete (mission finished)
  - [x] Cancelled
- [x] `ContinuousPayload` enum with IonBeam, GravityTractor, LaserAblation, SolarSail
- [x] `ThrustDirection` enum (Retrograde, Prograde, Radial, Custom)
- [x] `LaunchContinuousDeflectorEvent` for spawning deflectors
- [x] State transition system `update_deflector_states`
- [x] Progress tracking system `update_deflector_progress`

### 6.3 Physics Integration (`src/physics/mod.rs`)

- [x] `compute_continuous_thrust()` aggregation function
- [x] Modified `physics_step` to include thrust in acceleration
- [x] Thrust integrated with IAS15 adaptive integrator
- [x] Multiple deflectors can operate on same asteroid simultaneously

### 6.4 Prediction Integration (`src/prediction.rs`)

- [x] Modified `predict_trajectory` to query active deflectors
- [x] Thrust included in Velocity Verlet prediction loop
- [x] Trajectory preview accounts for continuous deflection

### 6.5 UI - Launch Controls (`src/ui/interceptor_launch.rs`)

- [x] Extended `PayloadType` enum with IonBeam, GravityTractor, LaserAblation, SolarSail
- [x] Ion Beam controls: thrust (mN), fuel mass (kg), Isp (s)
- [x] Gravity Tractor controls: spacecraft mass (kg), mission duration (years)
- [x] Laser Ablation controls: power (kW), mission duration (months)
- [x] Solar Sail controls: sail area (m²), reflectivity (0-1), mission duration (years)
- [x] Launch event handling for continuous methods

### 6.6 Mission Status Panel (`src/ui/mission_status.rs`)

- [x] Shows when continuous deflectors are active
- [x] Displays: method type, state, fuel remaining, accumulated Δv
- [x] Progress bar for fuel consumption
- [x] Color-coded icons per method type

### 6.7 Visualization (`src/render/deflectors.rs`)

- [x] En route: dashed line from Earth to asteroid
- [x] Operating: diamond spacecraft icon + thrust arrow
- [x] Completed: small circle icon
- [x] Color coding: cyan (ion), purple (gravity tractor), orange (laser), yellow/gold (solar sail)
- [x] NuclearSplit interceptor trajectory: red/pink color (danger warning)

### 6.8 Integration Tests

- [x] `examples/test_ion_beam_deflection.rs`:
  - [x] Basic deflection effect verification
  - [x] Delta-v accumulation matches physics
  - [x] Fuel depletion behavior
  - [x] Trajectory deviation measurement
- [x] `examples/test_gravity_tractor.rs`:
  - [x] Gravitational attraction (inverse-square law)
  - [x] Reference case validation (20,000 kg at 200m)
  - [x] Long-duration deflection (years of operation)

**Phase 6 Acceptance:** ✓ All three continuous deflection methods implemented with correct physics. Thrust integrated into physics loop and prediction system. UI supports launching continuous missions with configurable parameters. Mission status panel shows active deflectors. Visualization renders spacecraft and thrust direction. Integration tests verify physics calculations.

---

## Final Checklist

- [x] All unit tests pass (`cargo test`)
- [ ] No compiler warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [x] All phases complete (Phase 0-6)
- [ ] README updated with usage instructions

---

## Cross-Reference

| Topic | Document | Serena Memory |
|-------|----------|---------------|
| Architecture decisions | `ARCHITECTURE.md` | `design-decisions.md` |
| Physics & integrator | `PHYSICS.md` | - |
| Orbital elements data | `EPHEMERIS.md` | - |
| UI layouts and behavior | `UI.md` | - |
| Implementation order | `ROADMAP.md` | - |
| This checklist | `CHECKLIST.md` | - |

## Design Changes Log

| Date | Change | Rationale |
|------|--------|-----------|
| 2026-01 | Removed visual distortion | Trajectory accuracy > visual tidiness; Z-ordering suffices |
| 2026-01 | Moons decorative only | No gravity, no collision; simplifies physics model |
| 2026-01 | Proximity cap optimization | Only activates within 3× collision radius |
| 2026-01 | Phase 5 expanded with deflection | Added interceptor system (kinetic/nuclear) for planetary defense gameplay |
| 2026-01 | 6 scenarios instead of 4 | Added Deflection Challenge and expanded Apophis/Interstellar based on research |
| 2026-01 | Outcome detection system | Three outcomes (collision/escape/capture) with distinct visual feedback |
| 2026-01 | Phase 5/6 split for deflection | Instant methods (kinetic/nuclear) in Phase 5; continuous methods (ion beam, gravity tractor, laser) deferred to Phase 6 |
| 2026-01 | DART-based physics | Delta-v calculations use real DART mission beta factor (3.6) and LLNL nuclear research |
| 2026-01 | Phase 6 continuous deflection | Added Ion Beam Shepherd, Gravity Tractor, Laser Ablation with physics integration |
| 2026-01 | Solar Sail deflection method | 4th continuous method using solar radiation pressure (~9.08 μN/m² at 1 AU) |
| 2026-01 | NuclearSplit payload | Armageddon-style asteroid splitting; creates two fragments with diverging trajectories |
| 2026-01 | Asteroid mass editor | Collapsible UI in info panel with logarithmic slider (10^6-10^15 kg) and presets |
| 2026-01 | Deflection Challenge rebalanced | Reduced asteroid mass from 5e11 to 5e9 kg; expanded kinetic/nuclear slider ranges |
| 2026-01 | Interceptor speed inflation | BASE_INTERCEPTOR_SPEED: 15 km/s → 100 km/s for reasonable flight times (days not months) |
| 2026-01 | Deflection parameter boost | Kinetic: 50t/β=20 → 200t/β=40 (8 m/s); Nuclear: 2 MT → 12 MT (36 m/s) for multi-launch scenarios |
| 2026-01 | Default asteroid mass | Dock placement: 1e12 → 3e10 kg to match deflection tuning baseline |

> **Tip:** When starting a new coding session, run `list_memories` to see available context, then `read_memory` for relevant memories before beginning work.
