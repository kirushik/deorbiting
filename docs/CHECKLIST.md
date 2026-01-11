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
- [ ] Create Bevy `App` with default plugins
- [ ] Add orthographic camera:
  ```rust
  Camera3dBundle {
      projection: Projection::Orthographic(ortho),
      ..default()
  }
  ```
- [ ] Set initial camera position (z=1000 or appropriate)
- [ ] Add `Ephemeris` resource to app
- [ ] Add `SimulationTime` resource with current date

### 1.2 Camera System (`src/camera.rs`)
- [ ] Create `CameraPlugin`
- [ ] Implement zoom system:
  - [ ] Read mouse scroll wheel input
  - [ ] Apply logarithmic zoom (multiply/divide by factor)
  - [ ] Clamp to min/max zoom levels
- [ ] Implement pan system:
  - [ ] Detect middle mouse or left mouse on background
  - [ ] Convert screen delta to world delta
  - [ ] Update camera position
- [ ] Implement focus system:
  - [ ] Detect double-click on entity
  - [ ] Smoothly move camera to center on entity
- [ ] Define zoom constants (MIN_ZOOM, MAX_ZOOM, ZOOM_SPEED)

### 1.3 Celestial Body Spawning (`src/render/bodies.rs`)
- [ ] Create `CelestialBodyPlugin`
- [ ] Define `CelestialBody` component (radius, visual_scale, name)
- [ ] Create `spawn_solar_system()` startup system:
  - [ ] Spawn Sun entity with mesh and material
  - [ ] Spawn each planet entity
  - [ ] Register each entity in `Ephemeris` resource
- [ ] Create sphere meshes for each body:
  - [ ] Size based on `radius * visual_scale`
  - [ ] Color approximating real appearance
- [ ] Spawn moons with parent reference

### 1.4 Position Sync System (`src/render/sync.rs`)
- [ ] Create system `sync_celestial_positions`:
  - [ ] Query all entities with `CelestialBody`
  - [ ] Get position from `Ephemeris::get_position(entity, time)`
  - [ ] Convert DVec2 (f64) to Vec3 (f32) for Transform
  - [ ] Apply appropriate z-layer (2.0 for celestial bodies)
- [ ] Add system to `Update` schedule

### 1.5 Visual Polish (`src/render/background.rs`)
- [ ] Create starfield background:
  - [ ] Spawn many small white dots at z=0
  - [ ] Random positions covering viewport
  - [ ] Or: use a textured quad
- [ ] Add simple directional light (from Sun direction)
- [ ] Create planetary rings for Saturn (flat disc mesh)
  - [ ] Optional: Uranus and Neptune rings

### 1.6 Time Advancement
- [ ] Create `advance_time` system:
  - [ ] Read `SimulationTime.scale` and `paused`
  - [ ] Add `delta_seconds * scale * SECONDS_PER_DAY` to `current`
- [ ] Run in `Update` schedule

**Phase 1 Acceptance:** App runs, shows Sun and planets orbiting correctly, camera zoom/pan works, planets at correct positions for current date.

---

## Phase 2: Split-World & GUI

### 2.1 bevy_egui Setup
- [ ] Add `EguiPlugin` to app
- [ ] Create `UiPlugin` module (`src/ui/mod.rs`)

### 2.2 Time Controls Panel (`src/ui/time_controls.rs`)
- [ ] Create bottom panel with `egui::TopBottomPanel::bottom`
- [ ] Add play/pause button (toggle `SimulationTime.paused`)
- [ ] Add time scale buttons (1x, 10x, 100x, 1000x)
- [ ] Display current simulation date/time formatted
- [ ] Add reset button (store initial time for reset)
- [ ] Style with semi-transparent dark background

### 2.3 Visual Distortion (`src/distortion.rs`)
- [ ] Implement `apply_visual_distortion()` function:
  ```rust
  pub fn apply_visual_distortion(
      obj_pos: DVec2,
      planet_pos: DVec2,
      phys_r: f64,
      visual_scale: f32,
  ) -> DVec2
  ```
- [ ] Create `find_nearest_planet()` helper
- [ ] Integrate into `sync_visuals` system (for non-planet objects)

### 2.4 Info Panel (`src/ui/info_panel.rs`)
- [ ] Create right side panel with `egui::SidePanel::right`
- [ ] Add collapse/expand button
- [ ] Display selected body name
- [ ] Display position (X, Y)
- [ ] Display velocity magnitude
- [ ] Add km/AU unit toggle
- [ ] Create `SelectedBody` resource to track selection

### 2.5 Body Selection
- [ ] Implement mouse picking:
  - [ ] Convert screen coords to world coords
  - [ ] Find nearest body within click radius
  - [ ] Update `SelectedBody` resource
- [ ] Add visual highlight for selected body (glow/ring)

**Phase 2 Acceptance:** Time controls work, can pause/play/change speed, info panel shows selected body data, visual distortion ready (no asteroid yet to test).

---

## Phase 3: Asteroid Physics

### 3.1 Asteroid Entity (`src/asteroid.rs`)
- [ ] Define `Asteroid` marker component
- [ ] Create `spawn_asteroid()` function:
  - [ ] Create entity with `BodyState`, `Asteroid`, mesh
  - [ ] Initial position near Earth (e.g., 1.01 AU)
  - [ ] Initial velocity for roughly circular orbit
- [ ] Add to startup or scenario loading

### 3.2 IAS15 Integrator (`src/physics/integrator.rs`)
- [ ] Define `IAS15State` struct:
  ```rust
  pub struct IAS15State {
      pub pos: DVec2,
      pub vel: DVec2,
      acc: DVec2,
      g: [DVec2; 7],
      b: [DVec2; 7],
      e: [DVec2; 7],
      pub dt: f64,
      dt_last_done: f64,
  }
  ```
- [ ] Implement Gauss-Radau constants (h values, r matrix, c matrix)
- [ ] Implement `IAS15State::new(pos, vel, acc, initial_dt)`
- [ ] Implement `IAS15State::step()`:
  - [ ] Predictor step at 7 substep points
  - [ ] Compute accelerations at each point
  - [ ] Corrector iteration until convergence
  - [ ] Update position and velocity
  - [ ] Estimate error and adapt timestep
- [ ] Implement `IAS15State::from_body_state()`
- [ ] Add error tolerance constant (1e-9 recommended)

### 3.3 Gravity Calculation (`src/physics/gravity.rs`)
- [ ] Implement `compute_acceleration(pos, time, ephemeris) -> DVec2`
- [ ] Sum gravitational acceleration from all bodies
- [ ] Handle singularity (skip if r < threshold)

### 3.4 Physics System (`src/physics/mod.rs`)
- [ ] Create `PhysicsPlugin`
- [ ] Create `physics_step` system:
  - [ ] Skip if `SimulationTime.paused`
  - [ ] Query asteroids with `BodyState`
  - [ ] Run IAS15 step(s) for time delta
  - [ ] Update `BodyState` with new pos/vel
- [ ] Run in `FixedUpdate` schedule
- [ ] Handle time scaling (multiple substeps if needed)

### 3.5 Asteroid Rendering
- [ ] Update `sync_visuals` to handle asteroids:
  - [ ] Query entities with `Asteroid` and `BodyState`
  - [ ] Apply visual distortion
  - [ ] Set Transform at z=3.0 (spacecraft layer)
- [ ] Create gray sphere mesh for asteroid

### 3.6 Collision Detection (`src/collision.rs`)
- [ ] Create `check_collisions` system:
  - [ ] Query asteroids
  - [ ] Call `Ephemeris::check_collision()`
  - [ ] If collision: set `SimulationTime.paused = true`
  - [ ] Store collision event for UI
- [ ] Define `CollisionEvent` event type

### 3.7 Physics Tests
- [ ] Test IAS15 energy conservation over 100 orbits
- [ ] Test circular orbit stability (asteroid around Sun)
- [ ] Test Earth-like orbit period matches ~365 days
- [ ] Compare trajectory to analytic Kepler solution

**Phase 3 Acceptance:** Asteroid orbits correctly, energy conserved over long runs, collision detection works, physics stable at all time scales.

---

## Phase 4: Trajectory Prediction

### 4.1 Prediction Resource (`src/prediction.rs`)
- [ ] Define `PredictionSettings` resource:
  ```rust
  #[derive(Resource)]
  pub struct PredictionSettings {
      pub max_steps: usize,
      pub max_time: f64,
      pub update_interval: u32,
  }
  ```
- [ ] Define `TrajectoryPath` component:
  ```rust
  #[derive(Component)]
  pub struct TrajectoryPath {
      pub points: Vec<(DVec2, f64)>,
  }
  ```

### 4.2 Prediction System
- [ ] Create `predict_trajectory` system:
  - [ ] Clone asteroid's `BodyState`
  - [ ] Create new `IAS15State` from clone
  - [ ] Run simulation loop for max_steps
  - [ ] Store (position, time) at each step
  - [ ] Stop early on collision or max_time
  - [ ] Update `TrajectoryPath` component

### 4.3 Trajectory Rendering (`src/render/trajectory.rs`)
- [ ] Create `draw_trajectory` system using Bevy Gizmos:
  - [ ] Query entities with `TrajectoryPath`
  - [ ] For each point: apply visual distortion
  - [ ] Draw line segments between consecutive points
  - [ ] Use z=1.0 (trajectory layer)
- [ ] Add color gradient (fade with time/distance)

### 4.4 Velocity Handle (`src/ui/velocity_handle.rs`)
- [ ] Create `VelocityHandle` component
- [ ] Spawn arrow mesh when asteroid selected:
  - [ ] Base at asteroid position
  - [ ] Length proportional to velocity (log scale)
  - [ ] Direction = velocity direction
- [ ] Implement drag interaction:
  - [ ] Detect click on arrow tip
  - [ ] Track drag delta
  - [ ] Update `BodyState.vel` in real-time
  - [ ] Trigger prediction recalculation
- [ ] Update arrow visual during drag

### 4.5 Prediction Updates
- [ ] Recalculate prediction when:
  - [ ] Velocity handle dragged
  - [ ] Asteroid selected
  - [ ] Every N frames (configurable)
- [ ] Run prediction in separate system, not blocking render

**Phase 4 Acceptance:** Trajectory line visible and accurate, velocity handle works, dragging updates prediction in real-time.

---

## Phase 5: Scenarios & Polish

### 5.1 Scenario System (`src/scenarios/mod.rs`)
- [ ] Define `Scenario` struct:
  ```rust
  pub struct Scenario {
      pub name: &'static str,
      pub description: &'static str,
      pub asteroid_pos: DVec2,
      pub asteroid_vel: DVec2,
      pub start_time: f64,
  }
  ```
- [ ] Create `load_scenario()` function:
  - [ ] Reset simulation time
  - [ ] Respawn/reset asteroid with scenario values
  - [ ] Clear trajectory

### 5.2 Preset Scenarios (`src/scenarios/presets.rs`)
- [ ] Create "Apophis Approach" scenario:
  - [ ] Near-Earth asteroid on close approach
- [ ] Create "Jupiter Gravity Assist" scenario:
  - [ ] Asteroid trajectory passing Jupiter
- [ ] Create "Earth-Moon System" scenario:
  - [ ] Asteroid in cislunar space
- [ ] Create "Sandbox" scenario:
  - [ ] Default position, zero velocity

### 5.3 Scenario Menu (`src/ui/scenario_menu.rs`)
- [ ] Create modal window with `egui::Window`
- [ ] List all scenarios with descriptions
- [ ] Radio button selection
- [ ] Load and Cancel buttons
- [ ] Trigger via menu button or Escape key

### 5.4 Impact Overlay (`src/ui/impact_overlay.rs`)
- [ ] Create full-screen overlay on collision:
  - [ ] Semi-transparent dark background
  - [ ] "IMPACT!" text centered
  - [ ] Show which body was hit
  - [ ] Reset Scenario / New Scenario buttons
- [ ] Trigger from `CollisionEvent`

### 5.5 Visual Effects
- [ ] Impact flash effect (brief screen flash)
- [ ] Trajectory line fade (alpha gradient)
- [ ] Selection highlight (ring or glow)

### 5.6 Keyboard Shortcuts
- [ ] Space: play/pause
- [ ] 1-4: time scale
- [ ] R: reset scenario
- [ ] Escape/M: scenario menu
- [ ] +/-: zoom

### 5.7 Final Polish
- [ ] Performance profiling (target 60 FPS)
- [ ] Window resize handling
- [ ] Error handling for edge cases
- [ ] Code documentation

**Phase 5 Acceptance:** Scenarios load correctly, impact detection shows overlay, keyboard shortcuts work, stable 60 FPS.

---

## Final Checklist

- [ ] All unit tests pass (`cargo test`)
- [ ] No compiler warnings (`cargo clippy`)
- [ ] Code formatted (`cargo fmt`)
- [ ] All phases complete
- [ ] README updated with usage instructions

---

## Cross-Reference

| Topic | Document | Serena Memory |
|-------|----------|---------------|
| Architecture decisions | `ARCHITECTURE.md` | `design-decisions.md` |
| IAS15 algorithm details | `PHYSICS.md` | - |
| Orbital elements data | `EPHEMERIS.md` | - |
| UI layouts and behavior | `UI.md` | - |
| Implementation order | `ROADMAP.md` | - |
| This checklist | `CHECKLIST.md` | - |

> **Tip:** When starting a new coding session, run `list_memories` to see available context, then `read_memory` for relevant memories before beginning work.
