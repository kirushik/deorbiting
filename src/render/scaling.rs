//! Hierarchical visual scaling and position distortion for celestial bodies.
//!
//! Ensures:
//! 1. Moons never appear larger than their parent planet (size hierarchy)
//! 2. Moons never appear inside their parent planet (position distortion)
//! 3. All bodies remain visible at all zoom levels

use bevy::prelude::*;
use std::collections::HashMap;

use crate::camera::{CameraState, VIEWPORT_HEIGHT};
use crate::ephemeris::data::CelestialBodyId;

use super::bodies::{CelestialBody, DistortionOffset, EffectiveVisualRadius};

// === Constants ===

/// Maximum moon size as fraction of parent's visual radius.
/// Ensures moons are clearly smaller than their parent planet.
pub const MAX_MOON_FRACTION: f32 = 0.4;

/// Margin between parent visual edge and closest moon (fraction of parent radius).
/// Provides clear visual separation.
pub const MARGIN_FRACTION: f32 = 0.15;

/// Minimum spacing between consecutive moons (render units / Gm).
pub const MIN_MOON_SPACING: f32 = 0.3;

// === System Set ===

/// System set label for scaling and distortion (runs after sync, before orbit drawing).
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScalingDistortionSet;

// === Plugin ===

/// Plugin providing hierarchical visual scaling and position distortion.
pub struct ScalingPlugin;

impl Plugin for ScalingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScalingSettings>();
        // Systems are added by RenderPlugin with proper ordering
    }
}

/// Settings for dynamic body scaling.
#[derive(Resource)]
pub struct ScalingSettings {
    /// Minimum fraction of viewport height a body should occupy.
    pub min_screen_fraction: f32,

    /// Maximum scale multiplier applied to any body.
    pub max_scale: f32,
}

impl Default for ScalingSettings {
    fn default() -> Self {
        Self {
            min_screen_fraction: 0.004, // 0.4% of screen height minimum
            max_scale: 300.0,           // Cap maximum inflation
        }
    }
}

// === Hierarchical Scaling System ===

/// Two-pass hierarchical scaling system.
/// Pass 1: Compute scales for Sun and planets (no parent).
/// Pass 2: Compute scales for moons with parent-relative constraints.
pub fn compute_hierarchical_scales(
    camera_state: Res<CameraState>,
    settings: Res<ScalingSettings>,
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &CelestialBody,
        &mut EffectiveVisualRadius,
    )>,
) {
    let zoom = camera_state.zoom;
    let viewport_size = VIEWPORT_HEIGHT * zoom;
    let min_render_radius = viewport_size * settings.min_screen_fraction;

    // Store parent data for moon calculations
    let mut parent_data: HashMap<CelestialBodyId, (f32, f32)> = HashMap::new(); // (scale, effective_radius)

    // PASS 1: Sun and Planets (bodies without parents)
    for (_, mut transform, body, mut effective_radius) in bodies.iter_mut() {
        if body.id.parent().is_some() {
            continue; // Skip moons in pass 1
        }

        let base_radius = body.base_render_radius;

        let visibility_scale = if base_radius < min_render_radius {
            (min_render_radius / base_radius).min(settings.max_scale)
        } else {
            1.0
        };

        let final_scale = visibility_scale.max(1.0);
        let eff_radius = base_radius * final_scale;

        transform.scale = Vec3::splat(final_scale);
        effective_radius.0 = eff_radius;

        // Store for moon calculations
        parent_data.insert(body.id, (final_scale, eff_radius));
    }

    // PASS 2: Moons (bodies with parents)
    for (_, mut transform, body, mut effective_radius) in bodies.iter_mut() {
        let Some(parent_id) = body.id.parent() else {
            continue; // Skip non-moons in pass 2
        };

        let base_radius = body.base_render_radius;

        // Get parent's scale and radius
        let (parent_scale, parent_eff_radius) = parent_data
            .get(&parent_id)
            .copied()
            .unwrap_or((1.0, 1.0));

        // Calculate visibility scale as if independent
        let visibility_scale = if base_radius < min_render_radius {
            (min_render_radius / base_radius).min(settings.max_scale)
        } else {
            1.0
        };

        // CONSTRAINT 1: Inherit parent's scale factor as minimum
        // Moons scale proportionally with their parent
        let mut moon_scale = visibility_scale.max(parent_scale);

        // CONSTRAINT 2: Cap size at MAX_MOON_FRACTION of parent's visual radius
        let max_allowed_radius = parent_eff_radius * MAX_MOON_FRACTION;
        let moon_eff_radius = base_radius * moon_scale;

        if moon_eff_radius > max_allowed_radius {
            moon_scale = max_allowed_radius / base_radius;
        }

        let final_scale = moon_scale.max(1.0);
        let eff_radius = base_radius * final_scale;

        transform.scale = Vec3::splat(final_scale);
        effective_radius.0 = eff_radius;
    }
}

// === Position Distortion System ===

/// Push moons outward so they don't appear inside their visually-inflated parent.
pub fn apply_moon_position_distortion(
    mut bodies: Query<(
        Entity,
        &mut Transform,
        &CelestialBody,
        &EffectiveVisualRadius,
        &mut DistortionOffset,
    )>,
) {
    // Collect all body data first (to avoid borrow conflicts)
    let body_data: Vec<_> = bodies
        .iter()
        .map(|(e, t, b, r, _)| (e, t.translation.truncate(), b.id, r.0))
        .collect();

    // Group moons by parent and collect parent positions
    let mut parent_positions: HashMap<CelestialBodyId, (Vec2, f32)> = HashMap::new();
    let mut moons_by_parent: HashMap<CelestialBodyId, Vec<(Entity, Vec2, CelestialBodyId, f32)>> =
        HashMap::new();

    for (entity, pos, id, radius) in &body_data {
        if let Some(parent_id) = id.parent() {
            moons_by_parent
                .entry(parent_id)
                .or_default()
                .push((*entity, *pos, *id, *radius));
        } else {
            parent_positions.insert(*id, (*pos, *radius));
        }
    }

    // Process each planet's moons
    for (parent_id, moons) in moons_by_parent.iter_mut() {
        let Some(&(parent_pos, parent_radius)) = parent_positions.get(parent_id) else {
            continue;
        };

        // Sort moons by distance from parent (inner to outer)
        moons.sort_by(|a, b| {
            let dist_a = (a.1 - parent_pos).length();
            let dist_b = (b.1 - parent_pos).length();
            dist_a.partial_cmp(&dist_b).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Track the minimum distance for each subsequent moon
        let mut current_min_edge = parent_radius;

        for (moon_entity, moon_physics_pos, _moon_id, moon_radius) in moons.iter() {
            // Get the moon's transform mutably
            let Ok((_, mut transform, _, _, mut distortion_offset)) =
                bodies.get_mut(*moon_entity)
            else {
                continue;
            };

            let delta = *moon_physics_pos - parent_pos;
            let current_distance = delta.length();

            // Direction from parent to moon (or arbitrary if at center)
            let direction = if current_distance > 0.001 {
                delta.normalize()
            } else {
                Vec2::X
            };

            // Minimum distance: parent edge + moon radius + clearance
            // This ensures the moon's EDGE is outside the parent's EDGE
            let clearance = current_min_edge * MARGIN_FRACTION;
            let min_center_distance = current_min_edge + moon_radius + clearance;

            let (new_distance, offset) = if current_distance < min_center_distance {
                // Push outward
                (min_center_distance, min_center_distance - current_distance)
            } else {
                (current_distance, 0.0)
            };

            // Apply new position
            let new_pos = parent_pos + direction * new_distance;
            transform.translation.x = new_pos.x;
            transform.translation.y = new_pos.y;

            // Store distortion offset for potential future use
            distortion_offset.0 = direction * offset;

            // Update minimum edge for next moon (stacking effect)
            // Next moon must clear this moon's position
            current_min_edge = new_distance + moon_radius;
        }
    }
}
