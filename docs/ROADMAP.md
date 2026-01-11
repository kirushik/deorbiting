# Implementation Roadmap

## Phase 0: Foundation

1. **Dependencies:** Configure `Cargo.toml` with:
   - `bevy = "0.15"`
   - `bevy_egui` (check version compatibility with Bevy 0.15)

2. **Type System:** Create core physics types in `src/types.rs`:
   - `BodyState` with `DVec2` (f64) position/velocity
   - `SimulationTime` resource
   - Unit conversion utilities

3. **Ephemeris Data:** Implement in `src/ephemeris.rs`:
   - `KeplerOrbit` struct with Newton's method solver
   - Embed J2000 orbital elements as constants
   - Hierarchical orbit resolution for moons
   - `Ephemeris` resource for querying positions

4. **Unit Tests:** Write tests for:
   - Kepler solver accuracy (compare known positions)
   - Hierarchical orbit computation
   - Unit conversions

## Phase 1: The Static Universe

1. **Setup Bevy 0.15:** Initialize app in `src/main.rs`:
   - `Camera3dBundle` with `Projection::Orthographic`
   - Basic plugin structure

2. **Camera System:** Implement in `src/camera.rs`:
   - Mouse scroll wheel zoom (logarithmic scale)
   - Click-and-drag pan
   - Double-click to focus on body
   - Zoom range: ~50 AU to ~0.01 AU

3. **Planet Rendering:** Implement in `src/render/`:
   - Spawn Sun and 8 planets as spheres
   - Use `Ephemeris` to position them each frame
   - Color planets to resemble real counterparts
   - Verify orbital motion is correct

4. **Major Moons:** Add:
   - Earth's Moon
   - Jupiter's Galilean moons (Io, Europa, Ganymede, Callisto)
   - Saturn's Titan
   - Visual rings for Saturn, Uranus, Neptune (render-only)

5. **Visual Polish:**
   - Simple directional lighting
   - Starfield background

## Phase 2: The "Split-World" (Visual Distortion) and GUI

1. **Time Control:** Use `bevy_egui` to create bottom panel:
   - Play/pause button
   - Current simulation date/time display
   - Time scaling selector (1x, 10x, 100x, 1000x)
   - Reset to scenario start

2. **Distortion Algorithm:** Implement `apply_visual_distortion`:
   - Find nearest planet to object
   - Push visual position outward by (visual_radius - physical_radius)
   - Apply to all non-planet objects before rendering

3. **Info Panel:** Side panel with:
   - Selected body name and type
   - Position display (toggleable km/AU)
   - Velocity display (toggleable km·s⁻¹ / AU·day⁻¹)

## Phase 3: The Asteroid Physics

1. **Asteroid Setup:**
   - Create asteroid entity with `BodyState` (pos, vel, mass)
   - Render as small gray sphere
   - Initial placement near Earth

2. **Sync System:** Update `sync_visuals` to:
   - Query asteroid's `BodyState`
   - Apply visual distortion
   - Convert f64 position to f32 `Transform`

3. **IAS15 Integrator:** Implement in `src/physics/integrator.rs`:
   - Port from REBOUND or implement from paper
   - 15th-order Gauss-Radau quadrature
   - Adaptive timestep with error control
   - Run in `FixedUpdate` schedule

4. **Gravity Calculation:**
   - Sum gravitational forces from all celestial bodies
   - Use `Ephemeris::get_gravity_sources(time)`

5. **Verification:**
   - Test energy conservation over long runs
   - Verify stable Earth-like orbit
   - Compare trajectory to analytic solutions

## Phase 4: Trajectory Prediction

1. **Prediction Resource:** Create settings:
   - Maximum prediction steps
   - Maximum time horizon
   - Update frequency

2. **Simulation Loop:**
   - Clone asteroid state
   - Run IAS15 on clone for N steps
   - Query `Ephemeris` for future planet positions
   - Store results in `TrajectoryPath`

3. **Line Rendering:** Use Bevy Gizmos:
   - Draw lines between predicted points
   - Apply visual distortion to each point
   - Always visible (not just when paused)

4. **Velocity Handle:** Implement draggable arrow:
   - Arrow from asteroid center
   - Drag tip to set velocity vector
   - Length = speed (logarithmic scale)
   - Direction = heading
   - Real-time prediction updates while dragging

5. **Input Handling:**
   - Mouse picking for asteroid selection
   - Drag detection for velocity handle
   - Click-through to camera pan when not on UI

## Phase 5: Scenarios & Polish

1. **Preset Scenarios:** Create interesting starting configurations:
   - "Apophis-like" near-Earth asteroid approach
   - Comet on hyperbolic trajectory
   - Asteroid in Earth-Moon system
   - Jupiter gravity assist setup

2. **Scenario System:**
   - Save/load scenario definitions
   - Reset to scenario initial state
   - Sandbox mode for freeform placement

3. **Scenario Selector UI:**
   - Dropdown or modal to select preset
   - "New Sandbox" option
   - Brief description of each scenario

4. **Collision Effects:**
   - Detect when asteroid enters planet's physical radius
   - Pause simulation on impact
   - Display "Impact!" overlay
   - Visual effect (flash/explosion)
   - Manual reset or respawn options

5. **Polish:**
   - Trajectory line fade with distance/time
   - Planet labels
   - Orbit path visualization (optional toggle)
   - Performance optimization for smooth 60 FPS
