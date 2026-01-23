//! Visual effects for impacts and deflection events.
//!
//! Provides animated visual feedback for interceptor impacts:
//! - Kinetic impactor: white flash expanding circle
//! - Nuclear detonation: orange/yellow shockwave ring
//! - Nuclear split: two separating rings

use bevy::math::DVec2;
use bevy::prelude::*;

use crate::camera::RENDER_SCALE;

use super::z_layers;

/// Type of impact effect.
#[derive(Clone, Copy, Debug)]
pub enum ImpactEffectType {
    /// Kinetic impactor - white flash
    KineticFlash { intensity: f32 },
    /// Nuclear standoff detonation - expanding shockwave
    NuclearExplosion { yield_kt: f64 },
    /// Nuclear split - two separating rings
    NuclearSplit { yield_kt: f64 },
}

/// Component for animated impact effects.
#[derive(Component)]
pub struct ImpactEffect {
    /// Simulation time when effect started.
    pub start_time: f64,
    /// Duration of effect in simulation seconds.
    pub duration: f64,
    /// World position (physics coordinates, meters).
    pub position: DVec2,
    /// Type of effect to render.
    pub effect_type: ImpactEffectType,
}

/// Event to spawn an impact effect.
#[derive(Message)]
pub struct SpawnImpactEffectEvent {
    /// World position (meters).
    pub position: DVec2,
    /// Type of effect.
    pub effect_type: ImpactEffectType,
}

/// Spawn impact effects from events.
pub fn spawn_impact_effects(
    mut commands: Commands,
    mut events: MessageReader<SpawnImpactEffectEvent>,
    sim_time: Res<crate::types::SimulationTime>,
) {
    for event in events.read() {
        let duration = match &event.effect_type {
            ImpactEffectType::KineticFlash { .. } => 0.5,
            ImpactEffectType::NuclearExplosion { .. } => 2.0,
            ImpactEffectType::NuclearSplit { .. } => 3.0,
        };

        commands.spawn(ImpactEffect {
            start_time: sim_time.current,
            duration,
            position: event.position,
            effect_type: event.effect_type,
        });
    }
}

/// Animate and render impact effects.
pub fn animate_impact_effects(
    mut commands: Commands,
    effects: Query<(Entity, &ImpactEffect)>,
    sim_time: Res<crate::types::SimulationTime>,
    mut gizmos: Gizmos,
) {
    for (entity, effect) in effects.iter() {
        let elapsed = sim_time.current - effect.start_time;
        let progress = (elapsed / effect.duration).clamp(0.0, 1.0) as f32;

        // Despawn when complete
        if progress >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }

        // Convert position to render coordinates
        let center = Vec3::new(
            (effect.position.x * RENDER_SCALE) as f32,
            (effect.position.y * RENDER_SCALE) as f32,
            z_layers::SPACECRAFT + 0.2,
        );

        match &effect.effect_type {
            ImpactEffectType::KineticFlash { intensity } => {
                draw_kinetic_flash(&mut gizmos, center, progress, *intensity);
            }
            ImpactEffectType::NuclearExplosion { yield_kt } => {
                draw_nuclear_explosion(&mut gizmos, center, progress, *yield_kt);
            }
            ImpactEffectType::NuclearSplit { yield_kt } => {
                draw_nuclear_split(&mut gizmos, center, progress, *yield_kt);
            }
        }
    }
}

/// Draw kinetic impact flash - expanding white circle that fades.
fn draw_kinetic_flash(gizmos: &mut Gizmos, center: Vec3, progress: f32, intensity: f32) {
    // Expand quickly, fade out
    let radius = 0.02 + progress * 0.08;
    let alpha = (1.0 - progress) * intensity.min(1.0);

    let color = Color::srgba(1.0, 1.0, 1.0, alpha);

    // Draw expanding circle
    draw_circle_segments(gizmos, center, radius, color, 16);

    // Inner bright core (fades faster)
    let core_alpha = (1.0 - progress * 2.0).max(0.0) * intensity.min(1.0);
    if core_alpha > 0.0 {
        let core_color = Color::srgba(1.0, 1.0, 0.9, core_alpha);
        draw_circle_segments(gizmos, center, radius * 0.3, core_color, 8);
    }
}

/// Draw nuclear explosion - expanding orange/yellow shockwave ring.
fn draw_nuclear_explosion(gizmos: &mut Gizmos, center: Vec3, progress: f32, yield_kt: f64) {
    // Scale ring size based on yield (logarithmic)
    let yield_factor = (yield_kt.log10() / 4.0).clamp(0.5, 2.0) as f32;

    // Outer shockwave ring - expands and fades
    let outer_radius = 0.03 + progress * 0.15 * yield_factor;
    let outer_alpha = (1.0 - progress).powf(0.5);
    let outer_color = Color::srgba(1.0, 0.6, 0.1, outer_alpha);
    draw_circle_segments(gizmos, center, outer_radius, outer_color, 24);

    // Inner ring (yellow-white, fades faster)
    let inner_progress = (progress * 1.5).min(1.0);
    let inner_radius = 0.02 + inner_progress * 0.08 * yield_factor;
    let inner_alpha = (1.0 - inner_progress).max(0.0);
    let inner_color = Color::srgba(1.0, 0.9, 0.3, inner_alpha);
    draw_circle_segments(gizmos, center, inner_radius, inner_color, 16);

    // Central flash (white, very short)
    if progress < 0.2 {
        let flash_alpha = 1.0 - progress * 5.0;
        let flash_color = Color::srgba(1.0, 1.0, 1.0, flash_alpha);
        draw_circle_segments(gizmos, center, 0.01, flash_color, 8);
    }

    // Particle dots radiating outward
    draw_explosion_particles(gizmos, center, progress, yield_factor, outer_color, 12);
}

/// Draw nuclear split - two separating explosion rings.
fn draw_nuclear_split(gizmos: &mut Gizmos, center: Vec3, progress: f32, yield_kt: f64) {
    let yield_factor = (yield_kt.log10() / 4.0).clamp(0.5, 2.0) as f32;

    // Separation distance increases over time
    let separation = progress * 0.1 * yield_factor;

    // Two centers moving apart (perpendicular to typical deflection)
    let offset = Vec3::new(separation, 0.0, 0.0);
    let center1 = center + offset;
    let center2 = center - offset;

    // Ring parameters
    let radius = 0.02 + progress * 0.06 * yield_factor;
    let alpha = (1.0 - progress).powf(0.7);

    // First fragment - orange
    let color1 = Color::srgba(1.0, 0.5, 0.2, alpha);
    draw_circle_segments(gizmos, center1, radius, color1, 16);
    draw_explosion_particles(gizmos, center1, progress, yield_factor * 0.7, color1, 6);

    // Second fragment - yellow
    let color2 = Color::srgba(1.0, 0.7, 0.2, alpha);
    draw_circle_segments(gizmos, center2, radius, color2, 16);
    draw_explosion_particles(gizmos, center2, progress, yield_factor * 0.7, color2, 6);

    // Central connecting energy (fades quickly)
    if progress < 0.4 {
        let connect_alpha = (0.4 - progress) * 2.5;
        let connect_color = Color::srgba(1.0, 0.8, 0.4, connect_alpha);
        gizmos.line(center1, center2, connect_color);
    }
}

/// Draw a circle using line segments.
fn draw_circle_segments(
    gizmos: &mut Gizmos,
    center: Vec3,
    radius: f32,
    color: Color,
    segments: usize,
) {
    let angle_step = std::f32::consts::TAU / segments as f32;

    for i in 0..segments {
        let angle1 = i as f32 * angle_step;
        let angle2 = (i + 1) as f32 * angle_step;

        let p1 = center + Vec3::new(angle1.cos() * radius, angle1.sin() * radius, 0.0);
        let p2 = center + Vec3::new(angle2.cos() * radius, angle2.sin() * radius, 0.0);

        gizmos.line(p1, p2, color);
    }
}

/// Draw particle dots radiating outward from explosion center.
fn draw_explosion_particles(
    gizmos: &mut Gizmos,
    center: Vec3,
    progress: f32,
    scale: f32,
    base_color: Color,
    count: usize,
) {
    let angle_step = std::f32::consts::TAU / count as f32;
    let particle_distance = progress * 0.12 * scale;
    let particle_alpha = (1.0 - progress).powf(1.5);

    // Vary particle color slightly
    let Srgba {
        red, green, blue, ..
    } = base_color.to_srgba();
    let particle_color = Color::srgba(red, green * 0.9, blue * 0.7, particle_alpha * 0.8);

    for i in 0..count {
        // Offset angle slightly for visual interest
        let angle = i as f32 * angle_step + progress * 0.5;
        let distance = particle_distance * (0.8 + 0.4 * ((i as f32 * 1.7).sin() * 0.5 + 0.5));

        let particle_pos = center + Vec3::new(angle.cos() * distance, angle.sin() * distance, 0.0);

        // Draw small dot (short line to itself creates a point-like effect)
        let dot_size = 0.003 * (1.0 - progress * 0.5);
        gizmos.line(
            particle_pos - Vec3::new(dot_size, 0.0, 0.0),
            particle_pos + Vec3::new(dot_size, 0.0, 0.0),
            particle_color,
        );
        gizmos.line(
            particle_pos - Vec3::new(0.0, dot_size, 0.0),
            particle_pos + Vec3::new(0.0, dot_size, 0.0),
            particle_color,
        );
    }
}
