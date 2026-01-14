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
use crate::ephemeris::Ephemeris;
use crate::physics::IntegratorStates;
use crate::render::z_layers;
use crate::types::{BodyState, SimulationTime, AU_TO_METERS, G};

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

/// Spawn a test asteroid near Earth in a roughly circular orbit.
///
/// Places the asteroid at 1.01 AU from the Sun (just outside Earth's orbit)
/// with the appropriate velocity for a circular orbit.
///
/// # Arguments
/// * `commands` - Bevy commands for entity spawning
/// * `meshes` - Asset storage for meshes
/// * `materials` - Asset storage for materials
/// * `counter` - Counter resource for generating unique names
/// * `ephemeris` - Ephemeris resource for computing orbital velocity
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
    // Position: 1.01 AU from Sun (just outside Earth's orbit)
    let distance = 1.01 * AU_TO_METERS;
    let pos = DVec2::new(distance, 0.0);

    // Get Sun's GM from ephemeris (first gravity source)
    // Fallback to Sun mass * G if not available
    let sun_gm = ephemeris
        .get_gravity_sources(time)
        .first()
        .map(|(_, gm)| *gm)
        .unwrap_or(G * 1.989e30);

    // Circular orbit velocity: v = sqrt(GM/r)
    let orbital_vel = (sun_gm / distance).sqrt();
    let vel = DVec2::new(0.0, orbital_vel);

    info!(
        "Spawning test asteroid at {:.4} AU with velocity {:.2} km/s",
        distance / AU_TO_METERS,
        orbital_vel / 1000.0
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

    // Clear collision state
    collision_state.clear();

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
