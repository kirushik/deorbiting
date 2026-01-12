//! Background rendering for the solar system visualization.
//!
//! Provides lighting systems. Background is solid black (default clear color).
//! Starfield and other visual polish will be added after visual distortion is implemented.

use bevy::prelude::*;

/// Plugin providing background visual elements.
pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_lighting);
    }
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
