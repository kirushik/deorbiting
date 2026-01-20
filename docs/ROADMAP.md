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

## Phase 2: GUI and Visual System

1. **Time Control:** Use `bevy_egui` to create bottom panel:
   - Play/pause button
   - Current simulation date/time display
   - Time scaling selector (1x, 10x, 100x, 1000x)
   - Reset to scenario start

2. **Visual Rendering:** (Distortion removed - see Design Changes)
   - Planets rendered with inflated visual scales for visibility
   - Asteroids render at true physics positions (no distortion)
   - Z-ordering ensures asteroids (z=3.0) always visible on top of planets (z=2.0)
   - Trajectory accuracy prioritized over visual tidiness

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
   - Render at true physics position (no distortion)
   - Convert f64 position to f32 `Transform`
   - Apply z=3.0 to ensure visibility on top of planets

3. **Integrator:** Implement in `src/physics/integrator.rs`:
   - Velocity Verlet (symplectic, 2nd order) with adaptive timestep
   - Adaptive timestep based on acceleration error estimation
   - Run in `FixedUpdate` schedule
   - (IAS15 can be implemented later for higher precision if needed)

4. **Gravity Calculation:**
   - Sum gravitational forces from 9 bodies: Sun + 8 planets
   - Moons are decorative only (no gravity contribution)
   - Use `Ephemeris::get_gravity_sources(time)` - fixed-size array

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
   - Draw lines between predicted points at true physics positions
   - Color segments based on gravitationally dominant body
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

---

## Phase 6: Advanced Deflection

1. **Continuous Deflection Methods:**
   - Ion Beam Shepherd (hovering spacecraft with ion exhaust)
   - Gravity Tractor (gravitational pull from nearby spacecraft)
   - Laser Ablation (surface vaporization for thrust)
   - Solar Sail (radiation pressure deflection)

2. **Additional Instant Payloads:**
   - Nuclear Split (Armageddon-style asteroid splitting into fragments)

3. **Physics Integration:**
   - Thrust integrated into IAS15 integrator
   - Trajectory prediction includes continuous deflection
   - Multiple simultaneous deflectors supported

4. **UI Enhancements:**
   - Launch controls for all deflection methods
   - Mission status panel for active continuous deflectors
   - Asteroid mass editor in info panel
   - En-route visualization for all deployments

---

## Design Changes (Post-Implementation)

| Change | Date | Rationale |
|--------|------|-----------|
| **Removed visual distortion** | 2026-01 | Caused trajectory/velocity mismatches; Z-ordering suffices for visibility |
| **Moons decorative only** | 2026-01 | No gravity contribution, no collision; simplifies physics model |
| **Optimized proximity timestep cap** | 2026-01 | Only activates within 3× collision radius to avoid unnecessary slowdown |
| **Collision zones** | 2026-01 | Planets: 50×, Sun: 2×, Moons: none |
| **Solar Sail method** | 2026-01 | Added 4th continuous deflection using solar radiation pressure |
| **Nuclear splitting** | 2026-01 | Armageddon-style asteroid breakup into two fragments |

See `docs/CHECKLIST.md` for detailed implementation notes.
