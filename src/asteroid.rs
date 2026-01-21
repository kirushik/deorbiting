//! Asteroid entity definition and spawning.
//!
//! Asteroids are the player-controlled objects that are simulated
//! using the IAS15 physics integrator. Unlike celestial bodies which
//! follow ephemeris-defined paths, asteroids have dynamic trajectories
//! computed in real-time.

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::camera::RENDER_SCALE;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::prediction::TrajectoryPath;
use crate::render::z_layers;
use crate::types::{AU_TO_METERS, BodyState, G};

/// Event to trigger a full simulation reset.
///
/// When fired, this event causes the simulation to:
/// - Reset time to initial value
/// - Despawn all asteroids (both user-spawned and initial)
/// - Spawn a fresh initial asteroid
/// - Clear collision state
///
/// This is the intended "clean slate" behavior since asteroid positions
/// are time-dependent (an asteroid at position X only makes sense at time T).
#[derive(Event)]
pub struct ResetEvent;

/// Marker component identifying an entity as a simulated asteroid.
///
/// Entities with this component will have their positions computed
/// by the physics integrator rather than read from ephemeris.
#[derive(Component, Default)]
pub struct Asteroid;

/// Name component for asteroid display in UI.
#[derive(Component, Clone, Debug)]
pub struct AsteroidName(pub String);

/// Resource for generating unique asteroid names.
#[derive(Resource, Default)]
pub struct AsteroidCounter(pub u32);

/// Color palette for differentiating asteroids.
/// Warm, earthy tones that are visible against dark space.
const ASTEROID_COLORS: [Color; 6] = [
    Color::srgb(0.75, 0.55, 0.40), // Terracotta
    Color::srgb(0.60, 0.65, 0.70), // Steel blue-gray
    Color::srgb(0.70, 0.60, 0.45), // Sandy brown
    Color::srgb(0.55, 0.60, 0.55), // Sage green-gray
    Color::srgb(0.65, 0.50, 0.50), // Dusty rose
    Color::srgb(0.70, 0.70, 0.55), // Khaki
];

/// Vibrant indicator colors for UI elements and rings.
/// More saturated versions that are easily distinguishable.
const ASTEROID_INDICATOR_COLORS: [Color; 6] = [
    Color::srgb(1.0, 0.55, 0.25),  // Bright orange (from terracotta)
    Color::srgb(0.35, 0.75, 1.0),  // Bright cyan (from steel blue)
    Color::srgb(1.0, 0.82, 0.25),  // Gold (from sandy brown)
    Color::srgb(0.25, 0.95, 0.55), // Bright green (from sage)
    Color::srgb(1.0, 0.40, 0.65),  // Bright pink (from dusty rose)
    Color::srgb(0.75, 1.0, 0.25),  // Lime (from khaki)
];

/// Get a color for an asteroid based on its index.
pub fn asteroid_color(index: u32) -> Color {
    ASTEROID_COLORS[(index as usize) % ASTEROID_COLORS.len()]
}

/// Get a vibrant indicator color for an asteroid based on its index.
/// Used for UI buttons and visibility rings.
pub fn asteroid_indicator_color(index: u32) -> Color {
    ASTEROID_INDICATOR_COLORS[(index as usize) % ASTEROID_INDICATOR_COLORS.len()]
}

/// Map a material color to its corresponding indicator color.
/// Returns the vibrant version of the given asteroid color.
pub fn indicator_color_from_material(material_color: Color) -> Color {
    // Find matching material color and return corresponding indicator
    for (i, &mat_color) in ASTEROID_COLORS.iter().enumerate() {
        // Compare RGB values (with tolerance for floating point)
        let mat_rgba = mat_color.to_srgba();
        let test_rgba = material_color.to_srgba();
        if (mat_rgba.red - test_rgba.red).abs() < 0.01
            && (mat_rgba.green - test_rgba.green).abs() < 0.01
            && (mat_rgba.blue - test_rgba.blue).abs() < 0.01
        {
            return ASTEROID_INDICATOR_COLORS[i];
        }
    }
    // Fallback: brighten the original color
    let rgba = material_color.to_srgba();
    Color::srgb(
        (rgba.red * 1.3).min(1.0),
        (rgba.green * 1.3).min(1.0),
        (rgba.blue * 1.3).min(1.0),
    )
}

/// Visual properties for asteroid rendering.
#[derive(Component, Clone, Debug)]
pub struct AsteroidVisual {
    /// Render radius in render units (for display and picking).
    pub render_radius: f32,
    /// Base color for the asteroid mesh.
    pub color: Color,
}

impl Default for AsteroidVisual {
    fn default() -> Self {
        Self {
            render_radius: 2.0,                // Visible size in render units
            color: Color::srgb(0.6, 0.6, 0.6), // Gray
        }
    }
}

/// Spawn an asteroid with the given initial conditions.
///
/// # Arguments
/// * `commands` - Bevy commands for entity spawning
/// * `meshes` - Asset storage for meshes
/// * `materials` - Asset storage for materials
/// * `name` - Display name for the asteroid
/// * `pos` - Initial position in meters from solar system barycenter
/// * `vel` - Initial velocity in m/s
/// * `mass` - Mass in kg (primarily for display; negligible for gravity)
/// * `color` - Color for this asteroid (from palette)
///
/// # Returns
/// The spawned asteroid's Entity ID
pub fn spawn_asteroid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    name: String,
    pos: DVec2,
    vel: DVec2,
    mass: f64,
    color: Color,
) -> Entity {
    let visual = AsteroidVisual {
        render_radius: 2.0,
        color,
    };

    // Create sphere mesh
    let mesh = meshes.add(Sphere::new(visual.render_radius));

    // Create material with asteroid's color
    let material = materials.add(StandardMaterial {
        base_color: visual.color,
        perceptual_roughness: 0.8,
        metallic: 0.1,
        ..default()
    });

    // Convert physics position to render position
    let render_pos = Vec3::new(
        (pos.x * RENDER_SCALE) as f32,
        (pos.y * RENDER_SCALE) as f32,
        z_layers::SPACECRAFT,
    );

    commands
        .spawn((
            Asteroid,
            AsteroidName(name),
            BodyState { pos, vel, mass },
            TrajectoryPath::default(),
            visual,
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform::from_translation(render_pos),
        ))
        .id()
}

/// Spawn an asteroid at a given position using the counter for naming.
///
/// This is the main function used by the UI to spawn new asteroids.
///
/// # Arguments
/// * `commands` - Bevy commands for entity spawning
/// * `meshes` - Asset storage for meshes
/// * `materials` - Asset storage for materials
/// * `counter` - Counter resource for generating unique names
/// * `pos` - Initial position in meters from solar system barycenter
/// * `vel` - Initial velocity in m/s
///
/// # Returns
/// The spawned asteroid's Entity ID
pub fn spawn_asteroid_at_position(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    counter: &mut ResMut<AsteroidCounter>,
    pos: DVec2,
    vel: DVec2,
) -> Entity {
    counter.0 += 1;
    let name = format!("Asteroid {}", counter.0);
    let mass = 1e12; // Default mass: 1 trillion kg
    let color = asteroid_color(counter.0);

    info!("Spawning {} at ({:.2e}, {:.2e}) m", name, pos.x, pos.y);

    spawn_asteroid(commands, meshes, materials, name, pos, vel, mass, color)
}

/// Calculate position and velocity for an Earth intercept trajectory.
///
/// Places the asteroid on Earth's exact elliptical orbit, traveling backwards.
/// This guarantees collision in approximately 23 days regardless of Earth's
/// orbital phase.
///
/// The key insight is that Earth has an elliptical orbit (e=0.0167). For
/// guaranteed collision, the asteroid must be on the SAME elliptical orbit
/// as Earth, just traveling in the opposite direction. This is achieved by:
/// 1. Computing where Earth will be 45° ahead in its orbit
/// 2. Placing asteroid at that exact point on Earth's orbital path
/// 3. Computing Earth's actual velocity at that point (including radial component)
/// 4. Reversing the velocity so asteroid travels backwards along Earth's orbit
///
/// Since both bodies are on the same orbital track moving toward each other,
/// collision is guaranteed.
///
/// # Arguments
/// * `ephemeris` - Ephemeris resource for computing Earth's position
/// * `time` - Current simulation time (seconds since J2000)
///
/// # Returns
/// Tuple of (position, velocity) in meters and m/s
pub fn calculate_earth_intercept(ephemeris: &Ephemeris, time: f64) -> (DVec2, DVec2) {
    // Time offset for 45° ahead in Earth's orbit
    // Earth moves at ~0.986°/day, so 45° = ~45.6 days
    let days_for_45_degrees = 45.0 / 0.9856;
    let time_offset = days_for_45_degrees * 86400.0;

    let future_time = time + time_offset;

    // Get Earth's position 45° ahead in its orbit
    let asteroid_pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, future_time)
        .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

    // Compute Earth's ACTUAL velocity at this position using numerical differentiation
    // This captures both tangential and radial components of the elliptical orbit
    let dt = 60.0; // 1 minute timestep for differentiation
    let pos_before = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, future_time - dt)
        .unwrap_or(asteroid_pos);
    let pos_after = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, future_time + dt)
        .unwrap_or(asteroid_pos);

    // Central difference gives Earth's velocity at this orbital position
    let earth_velocity = (pos_after - pos_before) / (2.0 * dt);

    // Retrograde: asteroid travels opposite to Earth's motion
    // This puts it on the same elliptical orbit, just going backwards
    let asteroid_vel = -earth_velocity;

    (asteroid_pos, asteroid_vel)
}

/// Calculate velocity for an asteroid at a given position to intercept Earth.
///
/// Strategy: Use a retrograde orbit that crosses Earth's orbital radius.
/// If spawn is outside Earth's orbit, use slightly faster than circular to fall inward.
/// If spawn is inside Earth's orbit, use slightly slower to drift outward.
/// The retrograde direction ensures head-on collision geometry.
///
/// # Arguments
/// * `pos` - The asteroid's spawn position in meters
/// * `ephemeris` - Ephemeris resource for computing Earth's position
/// * `time` - Current simulation time (seconds since J2000)
///
/// # Returns
/// Velocity vector in m/s
pub fn calculate_velocity_for_earth_intercept(
    pos: DVec2,
    ephemeris: &Ephemeris,
    time: f64,
) -> DVec2 {
    let asteroid_r = pos.length();
    let asteroid_angle = pos.y.atan2(pos.x);

    // Get Earth's position to determine relative geometry
    let earth_pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, time)
        .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
    let earth_r = earth_pos.length();

    // GM_sun from standard gravitational parameter
    let gm_sun = G * 1.989e30;

    // Calculate circular velocity at this radius
    let v_circular = (gm_sun / asteroid_r).sqrt();

    // Tangent direction (perpendicular to radius, prograde direction)
    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());

    // Adjust velocity to create orbit that crosses Earth's radius
    let velocity_factor = if asteroid_r > earth_r * 1.1 {
        // Outside Earth orbit: speed up slightly to fall inward (elliptical)
        1.1
    } else if asteroid_r < earth_r * 0.9 {
        // Inside Earth orbit: slow down slightly to rise outward
        0.9
    } else {
        // Near Earth's orbital radius: circular retrograde works well
        1.0
    };

    // Retrograde velocity with adjustment
    -tangent * v_circular * velocity_factor
}

/// Spawn a test asteroid on a collision course with Earth.
///
/// Places the asteroid at ~1.5 AU from the Sun and calculates velocity
/// to intercept Earth in approximately 6 months.
///
/// # Arguments
/// * `commands` - Bevy commands for entity spawning
/// * `meshes` - Asset storage for meshes
/// * `materials` - Asset storage for materials
/// * `counter` - Counter resource for generating unique names
/// * `ephemeris` - Ephemeris resource for computing orbital positions
/// * `time` - Current simulation time (seconds since J2000)
///
/// # Returns
/// The spawned asteroid's Entity ID
pub fn spawn_test_asteroid(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    counter: &mut ResMut<AsteroidCounter>,
    ephemeris: &Ephemeris,
    time: f64,
) -> Entity {
    // Calculate position and velocity for Earth collision course
    let (pos, vel) = calculate_earth_intercept(ephemeris, time);

    info!(
        "Spawning asteroid on Earth collision course at ({:.4}, {:.4}) AU with velocity {:.2} km/s",
        pos.x / AU_TO_METERS,
        pos.y / AU_TO_METERS,
        vel.length() / 1000.0
    );

    spawn_asteroid_at_position(commands, meshes, materials, counter, pos, vel)
}

/// Startup system to spawn an initial test asteroid.
pub fn spawn_initial_asteroid(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut counter: ResMut<AsteroidCounter>,
    ephemeris: Res<Ephemeris>,
    time: Res<crate::types::SimulationTime>,
) {
    spawn_test_asteroid(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut counter,
        &ephemeris,
        time.current,
    );
}

/// System to handle simulation reset.
///
/// When a `ResetEvent` is received:
/// Reloads the current scenario by sending a LoadScenarioEvent.
/// This gives a "clean slate" - all user modifications are lost.
pub fn handle_reset(
    mut reset_events: EventReader<ResetEvent>,
    mut load_events: EventWriter<crate::scenarios::LoadScenarioEvent>,
    current_scenario: Res<crate::scenarios::CurrentScenario>,
) {
    // Only process if there's a reset event
    if reset_events.read().next().is_none() {
        return;
    }

    // Clear any remaining reset events
    reset_events.clear();

    info!(
        "Resetting simulation - reloading scenario: {}",
        current_scenario.id
    );

    // Send event to reload the current scenario
    load_events.send(crate::scenarios::LoadScenarioEvent {
        scenario_id: current_scenario.id,
    });
}
