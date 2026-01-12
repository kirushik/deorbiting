//! Celestial body rendering and spawning.
//!
//! Handles the visual representation of Sun, planets, and moons.

use std::f32::consts::PI;

use bevy::{
    math::DVec2,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
};

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

        // Add rings to Saturn
        if id == CelestialBodyId::Saturn {
            spawn_saturn_rings(&mut commands, &mut meshes, &mut materials, entity, render_radius);
        }

        // Register entity in ephemeris for later position lookups
        ephemeris.register(entity, id);
    }

    info!("Spawned {} celestial bodies", all_bodies().len());
}

/// Spawn Saturn's rings as a child entity.
fn spawn_saturn_rings(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    saturn_entity: Entity,
    saturn_radius: f32,
) {
    // Ring dimensions relative to Saturn's radius
    let inner_radius = saturn_radius * 1.2;
    let outer_radius = saturn_radius * 2.3;

    // Create ring mesh (annulus)
    let ring_mesh = create_ring_mesh(inner_radius, outer_radius, 64);
    let mesh_handle = meshes.add(ring_mesh);

    // Semi-transparent tan/beige color for the rings
    let ring_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.75, 0.55, 0.7),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    // Saturn's axial tilt is about 26.7 degrees
    let tilt_angle = 26.7_f32.to_radians();

    // Spawn rings as child of Saturn
    commands.entity(saturn_entity).with_children(|parent| {
        parent.spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(ring_material),
            Transform::from_rotation(Quat::from_rotation_x(tilt_angle)),
        ));
    });

    info!("Spawned Saturn's rings");
}

/// Create a ring (annulus) mesh.
fn create_ring_mesh(inner_radius: f32, outer_radius: f32, segments: u32) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();

    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * 2.0 * PI;
        let (sin, cos) = angle.sin_cos();

        // Inner vertex
        positions.push([inner_radius * cos, inner_radius * sin, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([i as f32 / segments as f32, 0.0]);

        // Outer vertex
        positions.push([outer_radius * cos, outer_radius * sin, 0.0]);
        normals.push([0.0, 0.0, 1.0]);
        uvs.push([i as f32 / segments as f32, 1.0]);

        if i < segments {
            let base = i * 2;
            // First triangle
            indices.push(base);
            indices.push(base + 1);
            indices.push(base + 2);
            // Second triangle
            indices.push(base + 1);
            indices.push(base + 3);
            indices.push(base + 2);
        }
    }

    Mesh::new(PrimitiveTopology::TriangleList, default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
        .with_inserted_indices(Indices::U32(indices))
}
