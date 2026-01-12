//! Background rendering for the solar system visualization.
//!
//! Provides starfield and lighting systems.

use bevy::prelude::*;
use rand::Rng;

use crate::render::z_layers;

/// Plugin providing background visual elements.
pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_starfield, spawn_lighting));
    }
}

/// Spawn a starfield background with randomly placed stars.
fn spawn_starfield(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Material for stars - emissive white
    let star_material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::WHITE * 0.5,
        unlit: true,
        ..default()
    });

    // Small sphere mesh for stars
    let star_mesh = meshes.add(Sphere::new(0.3));

    let mut rng = rand::thread_rng();

    // Spawn stars in a large area around the solar system
    for _ in 0..500 {
        let x = rng.gen_range(-5000.0..5000.0);
        let y = rng.gen_range(-5000.0..5000.0);
        let scale = rng.gen_range(0.5..1.5);

        commands.spawn((
            Mesh3d(star_mesh.clone()),
            MeshMaterial3d(star_material.clone()),
            Transform::from_xyz(x, y, z_layers::BACKGROUND).with_scale(Vec3::splat(scale)),
        ));
    }

    info!("Spawned 500 background stars");
}

/// Spawn lighting for the scene.
fn spawn_lighting(mut commands: Commands) {
    // Ambient light for general visibility
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
    });

    // Directional light from "above" the ecliptic plane
    commands.spawn((
        DirectionalLight {
            illuminance: 5000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 0.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    info!("Scene lighting initialized");
}
