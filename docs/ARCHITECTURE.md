## Architectural Overview

### 1. Tech Stack & Constraints
 - Engine: Bevy (Rust).
 - Renderer: 3D Renderer with Camera3dBundle set to Projection::Orthographic.
 - Physics Dimensions: 2D (x, y) logic, simulated on a flat plane.
 - Visual Dimensions: 3D (x, y, z) for z-indexing layers.
 - Math: glam::Vec2 for physics, glam::Vec3 for rendering.

### 2. Core Architecture: The "Split-World" Pattern

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

### 3. Data Structures (ECS)
```rust
// PHYSICS COMPONENTS (The Truth)
// Do not query Transform for physics calculations.
#[derive(Component)]
struct BodyState {
    pos: Vec2,
    vel: Vec2,
    mass: f32,
}

// Deterministic orbital parameters for planets
#[derive(Component)]
struct CelestialBody {
    radius: f32,       // Physical radius
    visual_scale: f32, // How much to inflate visually
    // Keplerian elements or simple circular orbit data
    orbit_radius: f32,
    orbit_speed: f32,
    phase: f32,
}

// PREDICTION DATA
#[derive(Component)]
struct TrajectoryPath {
    // Stores position AND the specific time that position occurs
    // Necessary because distortion depends on where the planet IS at that specific time
    points: Vec<(Vec2, f32)>, 
}

// SINGLE SOURCE OF TRUTH
#[derive(Resource)]
struct Ephemeris;
// Must implement: fn get_planet_pos(&self, entity: Entity, time: f32) -> Vec2
```

### 4. The Visual Distortion Algorithm

To make the game playable, planets are rendered larger than their physics bodies. To prevent the asteroid from visually clipping inside a planet or looking like it's crashing when it's actually in a safe orbit, we apply a distortion to the asteroid's visual position based on its proximity to the nearest planet.

#### Logic:
 - Calculate distance between Asteroid and Planet (Physics Space).
 - Calculate the radius_delta (Visual Radius - Physical Radius).
 - Push the Asteroid's visual position away from the planet center by radius_delta.

```rust
// Reference Implementation for AI
fn apply_visual_distortion(obj_pos: Vec2, planet_pos: Vec2, phys_r: f32, visual_scale: f32) -> Vec2 {
    let delta = obj_pos - planet_pos;
    let dist = delta.length();
    let visual_r = phys_r * visual_scale;
    
    // If we are far away, the offset is constant (the radius difference)
    // If we are inside the planet, logic might need clamping, but for now:
    let radius_delta = visual_r - phys_r;
    
    let new_dist = dist + radius_delta; 
    let dir = delta.normalize_or_zero();
    
    planet_pos + (dir * new_dist)
}
```

### 5. Systems Pipeline
#### A. Update Loop (Play Mode)
1. physics_step: Updates BodyState.pos based on velocity and gravity.
2. sync_visuals:
   - Queries BodyState and Ephemeris.
   - Calculates the distorted position.
   - Writes to Transform.translation.

#### B. Prediction Loop (Pause/Edit Mode)
1. input_handling: User drags velocity handle -> Updates BodyState.vel.
2. predict_trajectory:
   - Clones current state.
   - Runs a fast loop (e.g., 500 ticks).
   - Calls Ephemeris to get gravity sources at future times $t+n$.
   - Stores results in TrajectoryPath.
3. draw_trajectory:
   - Iterates TrajectoryPath.
   - Crucial: For every point, looks up where the planet was at that point's time.
   - Applies apply_visual_distortion to the point.
   - Draws line using Gizmos.
