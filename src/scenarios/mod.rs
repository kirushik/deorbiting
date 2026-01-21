//! Scenario system for predefined educational simulations.
//!
//! Provides a collection of preset scenarios demonstrating different orbital mechanics concepts:
//! - Earth collision course
//! - Gravity assists (Apophis, Jupiter slingshot)
//! - Interstellar visitor (hyperbolic trajectory)
//! - Deflection challenge
//! - Sandbox mode

pub mod presets;

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::asteroid::{
    spawn_asteroid_at_position, Asteroid, AsteroidCounter,
};
use crate::camera::{MainCamera, RENDER_SCALE};
use crate::collision::CollisionState;
use crate::ephemeris::{CelestialBodyId, Ephemeris};
use crate::physics::IntegratorStates;
use crate::prediction::{mark_prediction_dirty, PredictionState, TrajectoryPath};
use crate::render::SelectedBody;
use crate::types::{SelectableBody, SimulationTime, AU_TO_METERS};
use crate::ui::ActiveNotification;

pub use presets::SCENARIOS;

/// Camera target for scenario initialization.
#[derive(Clone, Copy, Debug, Default)]
pub enum CameraTarget {
    /// Center on the Sun (default).
    #[default]
    Sun,
    /// Center on a specific celestial body.
    Body(CelestialBodyId),
    /// Center on the asteroid's initial position.
    Asteroid,
    /// Center on a specific position (meters).
    Position(DVec2),
}

/// A predefined scenario configuration.
#[derive(Clone, Copy, Debug)]
pub struct Scenario {
    /// Unique identifier for the scenario.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// Brief description of the scenario.
    pub description: &'static str,
    /// Initial asteroid position (meters). None = compute dynamically.
    pub asteroid_pos: Option<DVec2>,
    /// Initial asteroid velocity (m/s). None = compute dynamically.
    pub asteroid_vel: Option<DVec2>,
    /// Asteroid mass (kg).
    pub asteroid_mass: f64,
    /// Asteroid visual radius (render scale factor).
    pub asteroid_radius: f64,
    /// Start time offset from current (seconds). None = use current time.
    pub start_time: Option<f64>,
    /// Initial time scale.
    pub time_scale: f64,
    /// Whether to start paused.
    pub start_paused: bool,
    /// Camera target.
    pub camera_target: CameraTarget,
    /// Camera zoom level (orthographic scale).
    pub camera_zoom: f32,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            id: "sandbox",
            name: "Sandbox",
            description: "Free experimentation mode",
            asteroid_pos: None,
            asteroid_vel: None,
            asteroid_mass: 1e12,
            asteroid_radius: 5e6,
            start_time: None,
            time_scale: 1.0,
            start_paused: false,
            camera_target: CameraTarget::Sun,
            camera_zoom: 1.0,
        }
    }
}

/// Resource tracking the current active scenario.
#[derive(Resource, Default)]
pub struct CurrentScenario {
    /// ID of the current scenario.
    pub id: &'static str,
}

/// Resource for scenario menu state.
#[derive(Resource, Default)]
pub struct ScenarioMenuState {
    /// Whether the menu is open.
    pub open: bool,
    /// Currently selected scenario index (for radio buttons).
    pub selected_index: usize,
}

/// Event to trigger loading a scenario.
#[derive(Event)]
pub struct LoadScenarioEvent {
    /// ID of the scenario to load.
    pub scenario_id: &'static str,
}

/// Plugin providing scenario management.
pub struct ScenarioPlugin;

impl Plugin for ScenarioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentScenario>()
            .init_resource::<ScenarioMenuState>()
            .add_event::<LoadScenarioEvent>()
            .add_systems(Startup, load_default_scenario)
            .add_systems(Update, handle_load_scenario_event);
    }
}

/// Set the default scenario on startup.
fn load_default_scenario(mut current: ResMut<CurrentScenario>) {
    current.id = "earth_collision";
}

/// Handle scenario loading events.
#[allow(clippy::too_many_arguments)]
fn handle_load_scenario_event(
    mut commands: Commands,
    mut events: EventReader<LoadScenarioEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut counter: ResMut<AsteroidCounter>,
    mut sim_time: ResMut<SimulationTime>,
    mut collision_state: ResMut<CollisionState>,
    mut integrator_states: ResMut<IntegratorStates>,
    mut active_notification: ResMut<ActiveNotification>,
    mut current_scenario: ResMut<CurrentScenario>,
    mut prediction_state: ResMut<PredictionState>,
    mut selected_body: ResMut<SelectedBody>,
    mut camera_query: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
    asteroids: Query<Entity, With<Asteroid>>,
    trajectory_query: Query<Entity, With<TrajectoryPath>>,
    ephemeris: Res<Ephemeris>,
) {
    for event in events.read() {
        // Find the scenario by ID
        let Some(scenario) = SCENARIOS.iter().find(|s| s.id == event.scenario_id) else {
            warn!("Unknown scenario ID: {}", event.scenario_id);
            continue;
        };

        info!("Loading scenario: {} ({})", scenario.name, scenario.id);

        // 1. Despawn all asteroids and their trajectories
        for entity in asteroids.iter() {
            commands.entity(entity).despawn();
            integrator_states.remove(entity);
        }
        for entity in trajectory_query.iter() {
            // Trajectory components are attached to asteroids, but just in case
            if !asteroids.contains(entity) {
                commands.entity(entity).despawn();
            }
        }

        // 2. Reset counter
        counter.0 = 0;

        // 3. Clear collision state and notifications
        collision_state.clear();
        active_notification.current = None;

        // 4. Set simulation time
        if let Some(start_time) = scenario.start_time {
            sim_time.current = start_time;
            sim_time.initial = start_time;
        } else {
            // Keep current time but update initial
            sim_time.initial = sim_time.current;
        }
        sim_time.scale = scenario.time_scale;
        sim_time.paused = scenario.start_paused;

        // 5. Compute asteroid position/velocity
        let (pos, vel) = compute_scenario_asteroid_state(scenario, &ephemeris, sim_time.current);

        // 6. Spawn asteroid and select it
        let asteroid_entity = spawn_asteroid_at_position(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut counter,
            pos,
            vel,
        );

        // 7. Auto-select the asteroid so UI shows its info
        selected_body.body = Some(SelectableBody::Asteroid(asteroid_entity));

        // 8. Update current scenario resource
        current_scenario.id = scenario.id;

        // 9. Position camera
        if let Ok((mut transform, mut projection)) = camera_query.get_single_mut() {
            let target_pos = match &scenario.camera_target {
                CameraTarget::Sun => DVec2::ZERO,
                CameraTarget::Body(id) => ephemeris.get_position_by_id(*id, sim_time.current).unwrap_or(DVec2::ZERO),
                CameraTarget::Asteroid => pos,
                CameraTarget::Position(p) => *p,
            };

            // Convert to render coordinates
            let render_pos = target_pos * RENDER_SCALE;
            transform.translation.x = render_pos.x as f32;
            transform.translation.y = render_pos.y as f32;

            // Set zoom
            if let Projection::Orthographic(ref mut ortho) = *projection {
                ortho.scale = scenario.camera_zoom;
            }
        }

        // 10. Trigger trajectory recalculation
        mark_prediction_dirty(&mut prediction_state);

        info!(
            "Scenario loaded: asteroid at ({:.2}, {:.2}) AU, velocity {:.1} km/s",
            pos.x / AU_TO_METERS,
            pos.y / AU_TO_METERS,
            vel.length() / 1000.0
        );
    }
}

/// Compute asteroid position and velocity for a scenario.
///
/// Uses dynamic computation based on current planet positions to ensure
/// scenarios work correctly regardless of simulation start time.
pub fn compute_scenario_asteroid_state(
    scenario: &Scenario,
    ephemeris: &Ephemeris,
    time: f64,
) -> (DVec2, DVec2) {
    use crate::types::GM_SUN;

    // First check for scenario-specific dynamic computation
    match scenario.id {
        "earth_collision" => {
            // Dynamic: 45° ahead of Earth, retrograde → collision ~23 days
            crate::asteroid::calculate_earth_intercept(ephemeris, time)
        }

        "apophis_flyby" => {
            // Dynamic: Close Earth flyby trajectory
            // Position slightly ahead and outside Earth's orbit
            let earth_pos = ephemeris.get_position_by_id(CelestialBodyId::Earth, time)
                .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
            let earth_r = earth_pos.length();
            let earth_angle = earth_pos.y.atan2(earth_pos.x);

            // 5° ahead of Earth, slightly outside
            let flyby_angle = earth_angle + 0.09; // ~5°
            let flyby_r = earth_r * 1.02; // 2% outside Earth orbit

            let pos = DVec2::new(flyby_r * flyby_angle.cos(), flyby_r * flyby_angle.sin());

            // Retrograde velocity with slight inward component for close approach
            let v_circular = (GM_SUN / flyby_r).sqrt();
            let tangent = DVec2::new(-flyby_angle.sin(), flyby_angle.cos());
            let radial = DVec2::new(flyby_angle.cos(), flyby_angle.sin());
            // Retrograde + slight inward = close flyby
            let vel = -tangent * v_circular * 0.98 - radial * 1500.0;

            (pos, vel)
        }

        "jupiter_slingshot" => {
            // Dynamic: Gravity assist at Jupiter
            // For a speed BOOST, the asteroid must be AHEAD of Jupiter, moving SLOWER.
            // Jupiter catches up and pulls the asteroid forward, adding energy.
            // This is similar to how Voyager gained speed - passing behind Jupiter
            // in Jupiter's reference frame.
            let jupiter_pos = ephemeris.get_position_by_id(CelestialBodyId::Jupiter, time)
                .unwrap_or(DVec2::new(5.2 * AU_TO_METERS, 0.0));
            let jupiter_r = jupiter_pos.length();
            let jupiter_angle = jupiter_pos.y.atan2(jupiter_pos.x);

            // Start AHEAD of Jupiter in its orbit (17° ahead), at same orbital radius
            // "Ahead" means in the direction Jupiter is moving
            let ahead_angle = jupiter_angle + 0.3; // ~17° ahead
            let start_r = jupiter_r; // Same orbital radius as Jupiter
            let pos = DVec2::new(start_r * ahead_angle.cos(), start_r * ahead_angle.sin());

            // Velocity: prograde but SLOWER than circular
            // This means Jupiter will catch up to the asteroid
            let v_circular = (GM_SUN / start_r).sqrt();
            let tangent = DVec2::new(-ahead_angle.sin(), ahead_angle.cos());
            // 90% of circular velocity - Jupiter will catch up
            let vel = tangent * v_circular * 0.9;

            (pos, vel)
        }

        "interstellar_visitor" => {
            // Hyperbolic: high velocity from "outside" the solar system
            // This works regardless of planet positions
            let pos = DVec2::new(-3.0 * AU_TO_METERS, 4.0 * AU_TO_METERS);
            // ~40 km/s toward inner system
            let vel = DVec2::new(28_000.0, -28_000.0);
            (pos, vel)
        }

        "deflection_challenge" => {
            // Dynamic: Same approach as Earth Collision but with longer lead time
            // 91° ahead of Earth (retrograde) → collision ~46 days
            // Uses numerical differentiation to get Earth's actual velocity on its
            // elliptical orbit, ensuring collision at all orbital phases.
            let days_for_91_degrees = 91.0 / 0.9856;
            let time_offset = days_for_91_degrees * 86400.0;
            let future_time = time + time_offset;

            // Get Earth's position 91° ahead
            let pos = ephemeris.get_position_by_id(CelestialBodyId::Earth, future_time)
                .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));

            // Compute Earth's actual velocity via numerical differentiation
            let dt = 60.0;
            let pos_before = ephemeris.get_position_by_id(CelestialBodyId::Earth, future_time - dt)
                .unwrap_or(pos);
            let pos_after = ephemeris.get_position_by_id(CelestialBodyId::Earth, future_time + dt)
                .unwrap_or(pos);
            let earth_velocity = (pos_after - pos_before) / (2.0 * dt);

            // Retrograde: asteroid travels opposite to Earth
            let vel = -earth_velocity;

            (pos, vel)
        }

        "sandbox" => {
            // Near Earth, zero velocity - user sets orbit
            let earth_pos = ephemeris.get_position_by_id(CelestialBodyId::Earth, time)
                .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
            let earth_angle = earth_pos.y.atan2(earth_pos.x);

            // Place just outside Earth's orbit, same angle
            let sandbox_r = earth_pos.length() * 1.05;
            let pos = DVec2::new(sandbox_r * earth_angle.cos(), sandbox_r * earth_angle.sin());
            let vel = DVec2::ZERO;

            (pos, vel)
        }

        // Fallback: use static values from scenario if specified
        _ => match (scenario.asteroid_pos, scenario.asteroid_vel) {
            (Some(pos), Some(vel)) => (pos, vel),
            (Some(pos), None) => {
                let r = pos.length();
                let v_circular = (GM_SUN / r).sqrt();
                let tangent = DVec2::new(-pos.y, pos.x).normalize();
                (pos, tangent * v_circular)
            }
            (None, Some(vel)) => {
                let earth_pos = ephemeris.get_position_by_id(CelestialBodyId::Earth, time)
                    .unwrap_or(DVec2::new(AU_TO_METERS, 0.0));
                (earth_pos, vel)
            }
            (None, None) => {
                crate::asteroid::calculate_earth_intercept(ephemeris, time)
            }
        }
    }
}

/// Get a scenario by ID.
pub fn get_scenario(id: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|s| s.id == id)
}
