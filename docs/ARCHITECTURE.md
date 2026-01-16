## Architectural Overview

### 1. Tech Stack & Constraints
 - Engine: Bevy 0.15 (Rust).
 - Renderer: 3D Renderer with Camera3dBundle set to Projection::Orthographic.
 - Physics Dimensions: 2D (x, y) logic, simulated on a flat plane.
 - Visual Dimensions: 3D (x, y, z) for z-indexing layers.
 - Math: glam::DVec2 (f64) for physics, glam::Vec3 (f32) for rendering.
 - Precision: f64 for all physics calculations, converted to f32 only for Bevy Transform.

### 2. Units & Scale

#### Physics Units
 - Position: Meters (f64)
 - Velocity: Meters per second (f64)
 - Time: Seconds (f64)
 - Mass: Kilograms (f64)

#### Internal Storage
 - Full SI units (no AU scaling internally)
 - Positions stored as meters from solar system barycenter

#### Display Units (User-Facing)
 - Toggleable between:
   * Kilometers / km·s⁻¹ (human-readable)
   * Astronomical Units / AU·day⁻¹ (astronomical convention)

#### Conversion Pipeline
 - Physics (f64 meters) → Camera-relative offset → Render (f32 scaled units)
 - Camera zoom determines the render scale factor

### 3. Camera Controls

 - **Zoom:** Mouse scroll wheel (logarithmic scale)
 - **Pan:** Click and drag (middle mouse button, or left mouse on background)
 - **Focus:** Double-click on celestial body to center camera
 - **Zoom Range:** From full solar system (~50 AU field of view) to planetary close-up (~0.01 AU)

### 4. Core Architecture: The "Split-World" Pattern

We strictly decouple **Simulation Space** (Physics) from **Render Space** (Visuals). Planets are rendered with inflated visual scales for visibility, but asteroids render at their true physics positions.

#### Coordinate Systems
 1. Physics Space (The Truth):
    - stored in custom components (e.g., BodyState).
    - Units: Meters, Seconds.
    - Planets are point masses or small spheres.
    - Gravity computed from Sun + 8 planets only (moons are decorative).
 2. Render Space (Visuals):
    - stored in Bevy's Transform component.
    - Units: Screen-relative or Scaled Meters.
    - Planets are visually inflated (scaled up) for visibility at all zoom levels.
    - Asteroids render at true physics positions (no distortion).
    - Z-Layering ensures asteroids always visible on top of planets:
      * 0.0: Background/Grid
      * 1.0: Trajectory Lines
      * 2.0: Celestial Bodies
      * 3.0: Spacecraft/Player (always on top)
      * 4.0: UI Handles

### 5. Data Structures (ECS)
```rust
// PHYSICS COMPONENTS (The Truth)
// Do not query Transform for physics calculations.
#[derive(Component)]
struct BodyState {
    pos: DVec2,   // glam::DVec2 (f64) - meters from barycenter
    vel: DVec2,   // glam::DVec2 (f64) - meters per second
    mass: f64,    // kilograms
}

// Deterministic orbital parameters for planets
// See EPHEMERIS.md for full Keplerian elements
#[derive(Component)]
struct CelestialBody {
    radius: f64,        // Physical radius in meters
    visual_scale: f32,  // How much to inflate visually (render-only)
}

// PREDICTION DATA
#[derive(Component)]
struct TrajectoryPath {
    // Stores position AND the specific time that position occurs
    points: Vec<TrajectoryPoint>,
}

struct TrajectoryPoint {
    pos: DVec2,                          // Position in meters
    time: f64,                           // Simulation time in seconds
    dominant_body: Option<CelestialBodyId>,  // For trajectory coloring
}

// SINGLE SOURCE OF TRUTH FOR CELESTIAL POSITIONS
#[derive(Resource)]
struct Ephemeris {
    // Contains KeplerOrbit data for all celestial bodies
    // See EPHEMERIS.md for implementation details
}

impl Ephemeris {
    /// Get position of a celestial body at a given time
    fn get_position(&self, body: Entity, time: f64) -> DVec2 { ... }

    /// Get all gravitating bodies for force calculations.
    /// Returns fixed-size array of 9 sources: Sun + 8 planets.
    /// Moons are decorative only (no gravity contribution).
    fn get_gravity_sources(&self, time: f64) -> [(DVec2, f64); 9] { ... }  // (pos, GM)
}
```

### 6. Visual Rendering (No Distortion)

Planets are rendered with inflated visual scales for visibility at all zoom levels.
Asteroids and trajectories render at their **true physics positions** without any distortion.

#### Why No Distortion?

For an intuition-building physics simulator, **trajectory accuracy is more important than visual tidiness**:

 - Z-ordering ensures asteroids (z=3.0) always render on top of planets (z=2.0)
 - An asteroid appearing "inside" an inflated planet is educational - it shows the difference between visual scale and physical reality
 - Trajectories always match physics exactly, with velocity arrows pointing in the correct direction
 - Simpler code with fewer edge cases

#### Moons Are Decorative Only

Moons (Earth's Moon, Jupiter's Galilean moons, Titan) are rendered for visual interest but:
 - **Do not contribute to gravity calculations** (only Sun + 8 planets)
 - **Have no collision detection** (asteroids pass through them)
 - Can be rendered at any visual scale without affecting physics

### 7. Collision Detection

#### Detection
 - Check if asteroid enters the "danger zone" around Sun or planets
 - **Planets**: 50× physical radius (e.g., Earth's zone is ~320,000 km)
 - **Sun**: 2× physical radius (~1.4 million km)
 - **Moons**: No collision detection (decorative only)
 - Performed every physics integration step

#### Response
 - Pause simulation immediately
 - Display "Impact!" overlay with visual effect
 - User must manually reset or respawn asteroid

```rust
const COLLISION_MULTIPLIER: f64 = 50.0;  // For planets

fn check_collision(asteroid_pos: DVec2, body_pos: DVec2, body_radius: f64) -> bool {
    (asteroid_pos - body_pos).length() < body_radius * COLLISION_MULTIPLIER
}
```

### 8. Systems Pipeline

#### A. Update Loop (Play Mode)
1. `physics_step`: Runs integrator in FixedUpdate, updates BodyState.pos/vel based on gravity from Sun + 8 planets.
2. Collision detection is integrated into `physics_step` to ensure synchronized positions.
3. `sync_visuals`:
   - Queries BodyState (asteroids) and Ephemeris (celestial bodies).
   - Converts f64 physics positions to f32 Transform.translation.
   - Applies z-ordering (asteroids on top of planets).

#### B. Prediction Loop (Always Active)
1. `input_handling`: User drags velocity handle -> Updates BodyState.vel.
2. `predict_trajectory`:
   - Clones current state.
   - Runs integrator loop for N steps into the future.
   - Queries Ephemeris for gravity sources at future times.
   - Stores results in TrajectoryPath with dominant body info for coloring.
3. `draw_trajectory`:
   - Iterates TrajectoryPath.
   - Draws line segments at true physics positions using Bevy Gizmos.
   - Colors segments based on gravitationally dominant body.
