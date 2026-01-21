//! Background rendering for the solar system visualization.
//!
//! Provides lighting and cosmetic elements like the asteroid belt.

use bevy::prelude::*;

use crate::camera::RENDER_SCALE;
use crate::render::z_layers;
use crate::types::AU_TO_METERS;

/// Plugin providing background visual elements.
pub struct BackgroundPlugin;

impl Plugin for BackgroundPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AsteroidBelt>()
            .add_systems(Startup, (spawn_lighting, generate_asteroid_belt))
            .add_systems(Update, draw_asteroid_belt);
    }
}

/// Spawn lighting for the scene.
fn spawn_lighting(mut commands: Commands) {
    // Ambient light for general visibility
    commands.spawn(AmbientLight {
        color: Color::WHITE,
        brightness: 200.0,
        affects_lightmapped_meshes: true,
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

// === Asteroid Belt ===

/// Asteroid belt parameters (in AU).
const BELT_INNER_AU: f64 = 2.2;
const BELT_OUTER_AU: f64 = 3.2;
/// Number of asteroids to render.
const ASTEROID_COUNT: usize = 2000;

/// Resource storing pre-computed asteroid positions.
#[derive(Resource, Default)]
pub struct AsteroidBelt {
    /// Positions in render coordinates (x, y).
    positions: Vec<Vec2>,
}

/// Generate random asteroid positions in the belt region.
fn generate_asteroid_belt(mut belt: ResMut<AsteroidBelt>) {
    use std::f64::consts::TAU;

    // Simple deterministic pseudo-random generator (for reproducibility)
    let mut seed: u64 = 42;
    let mut rng = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };

    belt.positions.clear();
    belt.positions.reserve(ASTEROID_COUNT);

    let inner_m = BELT_INNER_AU * AU_TO_METERS;
    let outer_m = BELT_OUTER_AU * AU_TO_METERS;

    for _ in 0..ASTEROID_COUNT {
        // Random angle around the Sun
        let angle = rng() * TAU;

        // Random radius with sqrt distribution for uniform area density
        let r_normalized = rng().sqrt();
        let r_m = inner_m + r_normalized * (outer_m - inner_m);

        // Convert to render coordinates
        let x = (r_m * angle.cos() * RENDER_SCALE) as f32;
        let y = (r_m * angle.sin() * RENDER_SCALE) as f32;

        belt.positions.push(Vec2::new(x, y));
    }

    info!("Generated {} asteroid belt objects", ASTEROID_COUNT);
}

/// Draw asteroid belt as small dots.
fn draw_asteroid_belt(belt: Res<AsteroidBelt>, mut gizmos: Gizmos) {
    let color = Color::srgba(0.5, 0.45, 0.4, 0.6); // Dim brownish-gray

    for pos in &belt.positions {
        // Draw as small circles at fixed size
        gizmos.circle(
            Isometry3d::from_translation(Vec3::new(pos.x, pos.y, z_layers::TRAJECTORY - 0.2)),
            0.3, // Small fixed radius in render units
            color,
        );
    }
}
