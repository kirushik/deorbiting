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

We strictly decouple **Simulation Space** (Physics) from **Render Space** (Visuals) to allow for "Visual Distortion" (inflating planets for visibility without breaking orbits).

#### Coordinate Systems
 1. Physics Space (The Truth):
    - stored in custom components (e.g., BodyState).
    - Units: Meters, Seconds.
    - Planets are point masses or small spheres.
 2. Render Space (The Lie):
    - stored in Bevy's Transform component.
    - Units: Screen-relative or Scaled Meters.
    - Planets are visually inflated (scaled up).
    - Z-Layering:
      * 0.0: Background/Grid
      * 1.0: Trajectory Lines
      * 2.0: Celestial Bodies
      * 3.0: Spacecraft/Player
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
    // Necessary because distortion depends on where the planet IS at that specific time
    points: Vec<(DVec2, f64)>,  // (position in meters, time in seconds)
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

    /// Get all gravitating bodies for force calculations
    fn get_gravity_sources(&self, time: f64) -> Vec<(DVec2, f64)> { ... }  // (pos, mass)
}
```

### 6. The Visual Distortion Algorithm

To make the game playable, planets are rendered larger than their physics bodies. To prevent the asteroid from visually clipping inside a planet or looking like it's crashing when it's actually in a safe orbit, we apply a distortion to the asteroid's visual position based on its proximity to the nearest planet.

#### Logic:
 - Calculate distance between Asteroid and Planet (Physics Space).
 - Calculate the radius_delta (Visual Radius - Physical Radius).
 - Push the Asteroid's visual position away from the planet center by radius_delta.

```rust
// Reference Implementation
// Note: Only considers nearest planet. May cause visual discontinuities
// at boundaries between planets' influence zones - acceptable for v1.
fn apply_visual_distortion(
    obj_pos: DVec2,
    planet_pos: DVec2,
    phys_r: f64,
    visual_scale: f32
) -> DVec2 {
    let delta = obj_pos - planet_pos;
    let dist = delta.length();
    let visual_r = phys_r * (visual_scale as f64);

    // If we are far away, the offset is constant (the radius difference)
    // If we are inside the planet, logic might need clamping, but for now:
    let radius_delta = visual_r - phys_r;

    let new_dist = dist + radius_delta;
    let dir = delta.normalize_or_zero();

    planet_pos + (dir * new_dist)
}
```

### 7. Collision Detection

#### Detection
 - Check if asteroid position enters any planet's physical radius
 - Performed every physics step

#### Response
 - Pause simulation immediately
 - Display "Impact!" overlay with visual effect (explosion/flash)
 - User must manually reset or respawn asteroid

```rust
fn check_collision(asteroid_pos: DVec2, planet_pos: DVec2, planet_radius: f64) -> bool {
    (asteroid_pos - planet_pos).length() < planet_radius
}
```

### 8. Systems Pipeline

#### A. Update Loop (Play Mode)
1. `physics_step`: Runs IAS15 integrator, updates BodyState.pos/vel based on gravity.
2. `check_collisions`: Detects impacts, triggers pause if collision detected.
3. `sync_visuals`:
   - Queries BodyState and Ephemeris.
   - Calculates the distorted position.
   - Converts f64 to f32 and writes to Transform.translation.

#### B. Prediction Loop (Always Active)
1. `input_handling`: User drags velocity handle -> Updates BodyState.vel.
2. `predict_trajectory`:
   - Clones current state.
   - Runs IAS15 loop for N steps into the future.
   - Calls Ephemeris to get gravity sources at future times t+n.
   - Stores results in TrajectoryPath.
3. `draw_trajectory`:
   - Iterates TrajectoryPath.
   - Crucial: For every point, looks up where the planet was at that point's time.
   - Applies apply_visual_distortion to the point.
   - Draws line using Bevy Gizmos.
