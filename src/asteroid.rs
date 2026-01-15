//! Asteroid entity definition and spawning.
//!
//! Asteroids are the player-controlled objects that are simulated
//! using the IAS15 physics integrator. Unlike celestial bodies which
//! follow ephemeris-defined paths, asteroids have dynamic trajectories
//! computed in real-time.

use bevy::prelude::*;
use bevy::math::DVec2;

use crate::camera::RENDER_SCALE;
use crate::collision::CollisionState;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::prediction::TrajectoryPath;
use crate::render::z_layers;
use crate::types::{BodyState, SimulationTime, AU_TO_METERS, G};
use crate::ui::ActiveNotification;

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
            render_radius: 2.0, // Visible size in render units
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
) -> Entity {
    let visual = AsteroidVisual::default();

    // Create sphere mesh
    let mesh = meshes.add(Sphere::new(visual.render_radius));

    // Create material (non-emissive gray)
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

    info!(
        "Spawning {} at ({:.2e}, {:.2e}) m",
        name, pos.x, pos.y
    );

    spawn_asteroid(commands, meshes, materials, name, pos, vel, mass)
}

/// Calculate position and velocity for an Earth intercept trajectory.
///
/// Places the asteroid on a retrograde orbit at Earth's distance but 45 degrees
/// ahead. This creates a collision in approximately 23 days, providing fast
/// feedback for testing collision detection.
///
/// # Arguments
/// * `ephemeris` - Ephemeris resource for computing Earth's position
/// * `time` - Current simulation time (seconds since J2000)
///
/// # Returns
/// Tuple of (position, velocity) in meters and m/s
pub fn calculate_earth_intercept(ephemeris: &Ephemeris, time: f64) -> (DVec2, DVec2) {
    // Get Earth's current position
    let earth_pos = ephemeris
        .get_position_by_id(CelestialBodyId::Earth, time)
        .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

    // Earth's orbital parameters
    let earth_r = earth_pos.length();
    let earth_angle = earth_pos.y.atan2(earth_pos.x);

    // Place asteroid 45 degrees ahead of Earth at same orbital radius
    // This gives ~23 days to collision - fast feedback for testing
    let offset_angle = std::f64::consts::PI / 4.0; // 45 degrees ahead
    let asteroid_angle = earth_angle + offset_angle;
    let asteroid_r = earth_r; // Same radius as Earth

    let asteroid_pos = DVec2::new(
        asteroid_r * asteroid_angle.cos(),
        asteroid_r * asteroid_angle.sin(),
    );

    // Calculate retrograde circular velocity at this radius
    // GM_sun from standard gravitational parameter
    let gm_sun = G * 1.989e30;
    let v_circular = (gm_sun / asteroid_r).sqrt();

    // Tangent direction (perpendicular to radius, prograde direction)
    let tangent = DVec2::new(-asteroid_angle.sin(), asteroid_angle.cos());

    // Retrograde velocity (opposite to prograde)
    let vel = -tangent * v_circular;

    (asteroid_pos, vel)
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
/// 1. Despawns all existing asteroids
/// 2. Resets asteroid counter to 0
/// 3. Resets simulation time to initial
/// 4. Clears collision state
/// 5. Spawns a fresh initial asteroid
///
/// This gives a "clean slate" - all user modifications are lost.
#[allow(clippy::too_many_arguments)]
pub fn handle_reset(
    mut commands: Commands,
    mut reset_events: EventReader<ResetEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut counter: ResMut<AsteroidCounter>,
    mut sim_time: ResMut<SimulationTime>,
    mut collision_state: ResMut<CollisionState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut active_notification: ResMut<ActiveNotification>,
    asteroids: Query<Entity, With<Asteroid>>,
    ephemeris: Res<Ephemeris>,
) {
    // Only process if there's a reset event
    if reset_events.read().next().is_none() {
        return;
    }

    // Clear any remaining reset events
    reset_events.clear();

    info!("Resetting simulation...");

    // Despawn all asteroids
    for entity in asteroids.iter() {
        commands.entity(entity).despawn();
        integrator_states.remove(entity);
    }

    // Reset counter
    counter.0 = 0;

    // Reset simulation time
    sim_time.reset();

    // Clear collision state and active notification
    collision_state.clear();
    active_notification.current = None;

    // Spawn fresh initial asteroid (use initial time, not current)
    spawn_test_asteroid(
        &mut commands,
        &mut meshes,
        &mut materials,
        &mut counter,
        &ephemeris,
        sim_time.initial,
    );

    info!("Simulation reset complete");
}
