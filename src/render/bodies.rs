//! Celestial body rendering and spawning.
//!
//! Handles the visual representation of Sun, planets, and moons.

use bevy::{math::DVec2, prelude::*};

use crate::camera::RENDER_SCALE;
use crate::ephemeris::{
    data::{all_bodies, CelestialBodyId},
    Ephemeris,
};
use crate::render::z_layers;
use crate::types::SimulationTime;

/// Component marking an entity as a renderable celestial body.
#[derive(Component)]
pub struct CelestialBody {
    /// Identifier for this body.
    pub id: CelestialBodyId,
    /// Physical radius in meters.
    pub radius: f64,
    /// Rendering scale multiplier.
    pub visual_scale: f32,
    /// Human-readable name.
    pub name: String,
}

/// Plugin providing celestial body spawning functionality.
pub struct CelestialBodyPlugin;

impl Plugin for CelestialBodyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_solar_system);
    }
}

/// Get the approximate visual color for a celestial body.
fn body_color(id: CelestialBodyId) -> Color {
    match id {
        CelestialBodyId::Sun => Color::srgb(1.0, 0.95, 0.4),
        CelestialBodyId::Mercury => Color::srgb(0.6, 0.6, 0.6),
        CelestialBodyId::Venus => Color::srgb(0.9, 0.85, 0.7),
        CelestialBodyId::Earth => Color::srgb(0.2, 0.5, 0.8),
        CelestialBodyId::Mars => Color::srgb(0.8, 0.4, 0.2),
        CelestialBodyId::Jupiter => Color::srgb(0.8, 0.7, 0.6),
        CelestialBodyId::Saturn => Color::srgb(0.9, 0.85, 0.6),
        CelestialBodyId::Uranus => Color::srgb(0.6, 0.8, 0.9),
        CelestialBodyId::Neptune => Color::srgb(0.3, 0.5, 0.9),
        CelestialBodyId::Moon => Color::srgb(0.7, 0.7, 0.7),
        CelestialBodyId::Io => Color::srgb(0.9, 0.8, 0.3),
        CelestialBodyId::Europa => Color::srgb(0.85, 0.85, 0.8),
        CelestialBodyId::Ganymede => Color::srgb(0.6, 0.55, 0.5),
        CelestialBodyId::Callisto => Color::srgb(0.4, 0.4, 0.4),
        CelestialBodyId::Titan => Color::srgb(0.8, 0.6, 0.3),
    }
}

/// Spawn all celestial bodies in the solar system.
fn spawn_solar_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut ephemeris: ResMut<Ephemeris>,
    time: Res<SimulationTime>,
) {
    for body_data in all_bodies() {
        let id = body_data.id;

        // Calculate render radius:
        // Physical radius scaled to render units, then multiplied by visual_scale
        let render_radius = (body_data.radius * RENDER_SCALE) as f32 * body_data.visual_scale;

        // Minimum visible size to ensure small bodies are visible and clickable
        let render_radius = render_radius.max(0.5);

        // Get initial position from ephemeris
        let pos = ephemeris
            .get_position_by_id(id, time.current)
            .unwrap_or(DVec2::ZERO);
        let render_pos = Vec3::new(
            (pos.x * RENDER_SCALE) as f32,
            (pos.y * RENDER_SCALE) as f32,
            z_layers::CELESTIAL,
        );

        // Create sphere mesh
        let mesh = meshes.add(Sphere::new(render_radius));

        // Create material - Sun is emissive (glows)
        let color = body_color(id);
        let material = materials.add(StandardMaterial {
            base_color: color,
            emissive: if id == CelestialBodyId::Sun {
                color.to_linear() * 2.0
            } else {
                LinearRgba::BLACK
            },
            ..default()
        });

        // Spawn entity
        let entity = commands
            .spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                Transform::from_translation(render_pos),
                CelestialBody {
                    id,
                    radius: body_data.radius,
                    visual_scale: body_data.visual_scale,
                    name: id.name().to_string(),
                },
            ))
            .id();

        // Register entity in ephemeris for later position lookups
        ephemeris.register(entity, id);
    }

    info!("Spawned {} celestial bodies", all_bodies().len());
}
