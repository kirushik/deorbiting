//! Ephemeris module for computing celestial body positions.
//!
//! Runtime behavior:
//! - Prefer table-based ephemeris generated from JPL Horizons (when present in `assets/ephemeris/`).
//! - Fall back to analytic Keplerian orbits otherwise.
//!
//! Coordinate frame:
//! - 2D heliocentric (Sun at origin), J2000 ecliptic plane.

pub mod data;
pub mod horizons_tables;
pub mod kepler;
pub mod table;

#[cfg(test)]
mod proptest_ephemeris;

pub use data::{CelestialBodyData, CelestialBodyId, CelestialBodyTrivia, all_bodies, get_trivia};

use crate::types::G;
use bevy::math::DVec2;
use bevy::prelude::*;
use std::collections::HashMap;
use std::sync::RwLock;

/// Multiplier for planet collision detection radius.
///
/// For gameplay purposes, we detect collision when an asteroid enters
/// the "danger zone" around a celestial body, not just its physical surface.
/// This makes the game playable while representing realistic intervention thresholds.
///
/// At 50x, Earth's danger zone is ~320,000 km (about the Moon's orbital distance).
///
/// Note: The Sun uses a separate 2x multiplier (see `SUN_COLLISION_MULT` in
/// `get_gravity_sources_full`). This is because the Sun is already huge
/// (696,000 km radius), and a 2x multiplier creates a ~1.4M km danger zone
/// which is reasonable without making it dominate the inner solar system.
pub const COLLISION_MULTIPLIER: f64 = 50.0;

/// Total number of gravity sources in the solar system model.
/// 1 Sun + 8 Planets = 9 bodies (moons are decorative only)
pub const GRAVITY_SOURCE_COUNT: usize = 9;

/// A gravity source: position and GM (standard gravitational parameter).
/// GM = G * mass, in m³/s². Use directly: a = GM/r²
pub type GravitySource = (DVec2, f64);

/// A gravity source with its ID: (body_id, position, GM).
pub type GravitySourceWithId = (CelestialBodyId, DVec2, f64);

/// Fixed-size array of gravity sources (no heap allocation).
pub type GravitySources = [GravitySource; GRAVITY_SOURCE_COUNT];

/// Fixed-size array of gravity sources with IDs (no heap allocation).
pub type GravitySourcesWithId = [GravitySourceWithId; GRAVITY_SOURCE_COUNT];

/// A full gravity source with all data needed for physics calculations.
/// Includes ID, position, GM, and collision radius - everything needed
/// for gravity, dominant body detection, and collision checking in one lookup.
#[derive(Clone, Copy, Debug)]
pub struct GravitySourceFull {
    pub id: CelestialBodyId,
    pub pos: DVec2,
    pub gm: f64,
    pub collision_radius: f64,
}

/// Fixed-size array of full gravity sources (no heap allocation).
pub type GravitySourcesFull = [GravitySourceFull; GRAVITY_SOURCE_COUNT];

/// A constant (Δpos, Δvel) offset applied to a base ephemeris state.
#[derive(Clone, Copy, Debug, Default)]
struct StateOffset2 {
    dp: DVec2,
    dv: DVec2,
}

/// Pre-computed GM values for all bodies in standard order.
/// Order: Sun, Mercury, Venus, Earth, Mars, Jupiter, Saturn, Uranus, Neptune,
///        Moon, Io, Europa, Ganymede, Callisto, Titan.
type GmCache = [f64; GRAVITY_SOURCE_COUNT];

/// Resource providing ephemeris data for all celestial bodies.
#[derive(Resource)]
pub struct Ephemeris {
    /// Mapping from entity to celestial body ID
    entity_to_id: HashMap<Entity, CelestialBodyId>,
    /// Mapping from celestial body ID to entity
    id_to_entity: HashMap<CelestialBodyId, Entity>,
    /// Cached body data (masses, radii, and legacy Kepler elements)
    body_data: HashMap<CelestialBodyId, CelestialBodyData>,

    /// Optional high-accuracy ephemeris tables generated from JPL Horizons.
    horizons: Option<horizons_tables::HorizonsTables>,

    /// Continuity offsets used when we fall back from Horizons tables to Kepler past table end.
    ///
    /// For each body with a table, if `t > table.end_time()`, we compute the Kepler state at the
    /// table end and apply a (Δpos, Δvel) offset so that Kepler matches the table exactly at the
    /// boundary. This avoids discontinuities when table coverage expires (e.g. after ~200 years
    /// for outer planets and major moons).
    ///
    /// This is cached via a thread-safe lock so `get_position_by_id` / `get_velocity_by_id` can
    /// remain `&self` while still satisfying Bevy `Resource` bounds.
    horizons_fallback_offsets: RwLock<HashMap<CelestialBodyId, StateOffset2>>,

    /// Pre-computed GM values for all bodies (computed once at startup).
    gm_cache: GmCache,
}

impl Default for Ephemeris {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard body order for gravity sources array.
/// Only includes Sun + 8 planets (moons are decorative only).
const BODY_ORDER: [CelestialBodyId; GRAVITY_SOURCE_COUNT] = [
    CelestialBodyId::Sun,
    CelestialBodyId::Mercury,
    CelestialBodyId::Venus,
    CelestialBodyId::Earth,
    CelestialBodyId::Mars,
    CelestialBodyId::Jupiter,
    CelestialBodyId::Saturn,
    CelestialBodyId::Uranus,
    CelestialBodyId::Neptune,
];

impl Ephemeris {
    /// Create a new ephemeris with all celestial body data loaded.
    ///
    /// If `assets/ephemeris/*.bin` is present (generated by the exporter script),
    /// those tables will be used for higher accuracy. Otherwise, we fall back
    /// to the baked-in Keplerian elements.
    pub fn new() -> Self {
        let mut body_data = HashMap::new();
        for data in all_bodies() {
            body_data.insert(data.id, data);
        }

        let horizons = horizons_tables::HorizonsTables::load_from_assets_dir().ok();

        // Pre-compute GM values for all bodies (computed once, used forever)
        let mut gm_cache = [0.0; GRAVITY_SOURCE_COUNT];
        for (i, &id) in BODY_ORDER.iter().enumerate() {
            if let Some(data) = body_data.get(&id) {
                gm_cache[i] = G * data.mass;
            }
        }

        Self {
            entity_to_id: HashMap::new(),
            id_to_entity: HashMap::new(),
            body_data,
            horizons,
            horizons_fallback_offsets: RwLock::new(HashMap::new()),
            gm_cache,
        }
    }

    /// Register an entity as a celestial body.
    pub fn register(&mut self, entity: Entity, id: CelestialBodyId) {
        self.entity_to_id.insert(entity, id);
        self.id_to_entity.insert(id, entity);
    }

    /// Get the entity for a celestial body ID.
    pub fn get_entity(&self, id: CelestialBodyId) -> Option<Entity> {
        self.id_to_entity.get(&id).copied()
    }

    /// Get the celestial body ID for an entity.
    pub fn get_id(&self, entity: Entity) -> Option<CelestialBodyId> {
        self.entity_to_id.get(&entity).copied()
    }

    /// Get the body data for an entity.
    pub fn get_body_data(&self, entity: Entity) -> Option<&CelestialBodyData> {
        let id = self.entity_to_id.get(&entity)?;
        self.body_data.get(id)
    }

    /// Get the body data for a celestial body ID.
    pub fn get_body_data_by_id(&self, id: CelestialBodyId) -> Option<&CelestialBodyData> {
        self.body_data.get(&id)
    }

    /// Get the mass of a celestial body (kg).
    pub fn get_mass(&self, entity: Entity) -> Option<f64> {
        self.get_body_data(entity).map(|d| d.mass)
    }

    /// Get the physical radius of a celestial body (meters).
    pub fn get_radius(&self, entity: Entity) -> Option<f64> {
        self.get_body_data(entity).map(|d| d.radius)
    }

    /// Compute position of a celestial body at given time.
    ///
    /// # Arguments
    /// * `entity` - The entity representing the celestial body
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Position in meters from solar system barycenter, or None if entity not found.
    pub fn get_position(&self, entity: Entity, time: f64) -> Option<DVec2> {
        let id = self.entity_to_id.get(&entity)?;
        self.get_position_by_id(*id, time)
    }

    /// Compute position of a celestial body by ID at given time.
    pub fn get_position_by_id(&self, id: CelestialBodyId, time: f64) -> Option<DVec2> {
        // Prefer high-accuracy Horizons table ephemeris when available.
        //
        // If `time` is outside the table range, we fall back to Kepler but apply a per-body
        // offset so the transition is continuous at the boundary (for planets only).
        if let Some(h) = &self.horizons
            && let Some(tbl) = h.table(id)
        {
            let start = tbl.start_time();
            let end = tbl.end_time();

            if time >= start && time <= end {
                if let Ok(state) = tbl.sample(time) {
                    return Some(state.pos);
                }
                // If sampling failed for some unexpected reason, fall through to Kepler.
            } else if time > end {
                // Past coverage end: patched Kepler continuation.
                let base = self.get_kepler_position_by_id(id, time)?;

                // For moons, the offset approach doesn't work because their heliocentric
                // position depends on the parent's current position, not where the parent
                // was at table end. Use pure Kepler for moons outside table coverage.
                if id.parent().is_some() {
                    return Some(base);
                }

                // For planets: compute or reuse the (Δpos, Δvel) offset at the end boundary.
                let offset = self.get_or_compute_horizons_offset(id, end)?;
                return Some(base + offset.dp);
            }
            // For `time < start`, we intentionally fall back to Kepler without offsets.
            // Tables are forward-only by design; negative times are not guaranteed.
        }

        // No table available: pure Kepler model.
        self.get_kepler_position_by_id(id, time)
    }

    /// Compute velocity of a celestial body at given time.
    ///
    /// # Arguments
    /// * `entity` - The entity representing the celestial body
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Velocity in m/s, or None if entity not found.
    pub fn get_velocity(&self, entity: Entity, time: f64) -> Option<DVec2> {
        let id = self.entity_to_id.get(&entity)?;
        self.get_velocity_by_id(*id, time)
    }

    /// Compute velocity of a celestial body by ID at given time.
    pub fn get_velocity_by_id(&self, id: CelestialBodyId, time: f64) -> Option<DVec2> {
        // Prefer high-accuracy Horizons table ephemeris when available.
        //
        // If `time` is outside the table range, we fall back to Kepler but apply a per-body
        // offset so the transition is continuous at the boundary.
        if let Some(h) = &self.horizons
            && let Some(tbl) = h.table(id)
        {
            let start = tbl.start_time();
            let end = tbl.end_time();

            if time >= start && time <= end {
                if let Ok(state) = tbl.sample(time) {
                    return Some(state.vel);
                }
                // If sampling failed for some unexpected reason, fall through to Kepler.
            } else if time > end {
                // Past coverage end: patched Kepler continuation (C0/C1 at end).
                let base = self.get_kepler_velocity_by_id(id, time)?;

                // Compute or reuse the (Δpos, Δvel) offset at the end boundary.
                let offset = self.get_or_compute_horizons_offset(id, end)?;
                return Some(base + offset.dv);
            }
            // For `time < start`, we intentionally fall back to Kepler without offsets.
            // Tables are forward-only by design; negative times are not guaranteed.
        }

        // No table available: pure Kepler model.
        self.get_kepler_velocity_by_id(id, time)
    }

    /// Get all gravity sources at a given time.
    ///
    /// Returns positions and GM (μ = G·M) values, NOT masses.
    /// GM is the standard gravitational parameter in m³/s².
    /// Use directly in acceleration formula: a = GM/r² (no need to multiply by G).
    ///
    /// This function returns a fixed-size array to avoid heap allocations.
    /// Uses pre-computed GM cache and batched position queries for performance.
    ///
    /// If a body's position lookup fails, it is logged and the body's GM is set to 0
    /// (effectively excluding it from gravity calculations).
    ///
    /// # Arguments
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Fixed array of (position in meters, GM in m³/s²) pairs for all massive bodies.
    pub fn get_gravity_sources(&self, time: f64) -> GravitySources {
        let mut result: GravitySources = [(DVec2::ZERO, 0.0); GRAVITY_SOURCE_COUNT];

        // Sun is always at origin (index 0)
        result[0] = (DVec2::ZERO, self.gm_cache[0]);

        // Try batched table sampling for bodies 1-8 (planets only, moons are decorative)
        if let Some(h) = &self.horizons {
            let positions = h.sample_all_positions(time);
            for (i, table_pos) in positions.iter().enumerate() {
                let body_idx = i + 1; // Skip Sun
                if let Some(pos) = *table_pos {
                    result[body_idx] = (pos, self.gm_cache[body_idx]);
                } else {
                    // Fallback to individual query (outside table range or missing table)
                    let id = BODY_ORDER[body_idx];
                    if let Some(pos) = self.get_position_by_id(id, time) {
                        result[body_idx] = (pos, self.gm_cache[body_idx]);
                    } else {
                        // Position lookup failed - exclude from gravity (GM = 0)
                        warn_once!(
                            "Failed to get position for {:?} at time {:.1}, excluding from gravity",
                            id,
                            time
                        );
                        result[body_idx] = (DVec2::ZERO, 0.0);
                    }
                }
            }
        } else {
            // No tables: use Kepler for all bodies
            for i in 1..GRAVITY_SOURCE_COUNT {
                let id = BODY_ORDER[i];
                if let Some(pos) = self.get_position_by_id(id, time) {
                    result[i] = (pos, self.gm_cache[i]);
                } else {
                    // Position lookup failed - exclude from gravity (GM = 0)
                    warn_once!(
                        "Failed to get position for {:?} at time {:.1}, excluding from gravity",
                        id,
                        time
                    );
                    result[i] = (DVec2::ZERO, 0.0);
                }
            }
        }

        result
    }

    /// Get all gravity sources with their IDs at a given time.
    ///
    /// Similar to `get_gravity_sources`, but includes the body ID for each source.
    /// Useful for determining which body's gravity dominates at a position.
    ///
    /// This function returns a fixed-size array to avoid heap allocations.
    ///
    /// # Arguments
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Fixed array of (body_id, position in meters, GM in m³/s²) tuples.
    pub fn get_gravity_sources_with_id(&self, time: f64) -> GravitySourcesWithId {
        // Build the fixed array with IDs (Sun + 8 planets, moons are decorative only)
        [
            // 0: Sun (always at origin)
            self.gravity_source_with_id_for(CelestialBodyId::Sun, time),
            // 1-8: Planets
            self.gravity_source_with_id_for(CelestialBodyId::Mercury, time),
            self.gravity_source_with_id_for(CelestialBodyId::Venus, time),
            self.gravity_source_with_id_for(CelestialBodyId::Earth, time),
            self.gravity_source_with_id_for(CelestialBodyId::Mars, time),
            self.gravity_source_with_id_for(CelestialBodyId::Jupiter, time),
            self.gravity_source_with_id_for(CelestialBodyId::Saturn, time),
            self.gravity_source_with_id_for(CelestialBodyId::Uranus, time),
            self.gravity_source_with_id_for(CelestialBodyId::Neptune, time),
        ]
    }

    /// Helper to get a single gravity source with ID for a body.
    /// Returns GM=0 if position lookup fails.
    #[inline]
    fn gravity_source_with_id_for(&self, id: CelestialBodyId, time: f64) -> GravitySourceWithId {
        if id == CelestialBodyId::Sun {
            let gm = self.body_data.get(&id).map(|d| G * d.mass).unwrap_or(0.0);
            return (id, DVec2::ZERO, gm);
        }

        if let Some(pos) = self.get_position_by_id(id, time) {
            let gm = self.body_data.get(&id).map(|d| G * d.mass).unwrap_or(0.0);
            (id, pos, gm)
        } else {
            // Position lookup failed - exclude from gravity
            (id, DVec2::ZERO, 0.0)
        }
    }

    /// Get all gravity sources with full data in a SINGLE ephemeris pass.
    ///
    /// This method returns position, GM, body ID, AND collision radius for all
    /// gravity sources in one lookup. This is significantly more efficient than
    /// calling `get_gravity_sources()`, `get_gravity_sources_with_id()`, and
    /// performing collision checks separately (which would require 3x the
    /// ephemeris interpolations).
    ///
    /// Use this in trajectory prediction loops where you need all three pieces
    /// of information per timestep.
    ///
    /// If a body's position lookup fails, it is logged and the body's GM is set to 0
    /// (effectively excluding it from gravity calculations).
    ///
    /// # Arguments
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Fixed array of `GravitySourceFull` with id, position, GM, and collision radius.
    pub fn get_gravity_sources_full(&self, time: f64) -> GravitySourcesFull {
        // Sun collision radius uses 2x multiplier (Sun is already huge)
        const SUN_COLLISION_MULT: f64 = 2.0;

        let mut result: GravitySourcesFull = [GravitySourceFull {
            id: CelestialBodyId::Sun,
            pos: DVec2::ZERO,
            gm: 0.0,
            collision_radius: 0.0,
        }; GRAVITY_SOURCE_COUNT];

        // Sun at origin (index 0)
        let sun_radius = self
            .body_data
            .get(&CelestialBodyId::Sun)
            .map(|d| d.radius)
            .unwrap_or(6.96e8);
        result[0] = GravitySourceFull {
            id: CelestialBodyId::Sun,
            pos: DVec2::ZERO,
            gm: self.gm_cache[0],
            collision_radius: sun_radius * SUN_COLLISION_MULT,
        };

        // Planets (indices 1-8) - batch position lookup where possible
        if let Some(h) = &self.horizons {
            let positions = h.sample_all_positions(time);
            for (i, table_pos) in positions.iter().enumerate() {
                let body_idx = i + 1;
                let id = BODY_ORDER[body_idx];
                let collision_radius = self
                    .body_data
                    .get(&id)
                    .map(|d| d.radius * COLLISION_MULTIPLIER)
                    .unwrap_or(0.0);

                let pos_opt = table_pos.or_else(|| self.get_position_by_id(id, time));

                if let Some(pos) = pos_opt {
                    result[body_idx] = GravitySourceFull {
                        id,
                        pos,
                        gm: self.gm_cache[body_idx],
                        collision_radius,
                    };
                } else {
                    // Position lookup failed - exclude from gravity (GM = 0)
                    warn_once!(
                        "Failed to get position for {:?} at time {:.1}, excluding from gravity",
                        id,
                        time
                    );
                    result[body_idx] = GravitySourceFull {
                        id,
                        pos: DVec2::ZERO,
                        gm: 0.0,               // Exclude from gravity
                        collision_radius: 0.0, // No collision with missing body
                    };
                }
            }
        } else {
            // No tables: use Kepler for all bodies
            for i in 1..GRAVITY_SOURCE_COUNT {
                let id = BODY_ORDER[i];
                let collision_radius = self
                    .body_data
                    .get(&id)
                    .map(|d| d.radius * COLLISION_MULTIPLIER)
                    .unwrap_or(0.0);

                if let Some(pos) = self.get_position_by_id(id, time) {
                    result[i] = GravitySourceFull {
                        id,
                        pos,
                        gm: self.gm_cache[i],
                        collision_radius,
                    };
                } else {
                    // Position lookup failed - exclude from gravity (GM = 0)
                    warn_once!(
                        "Failed to get position for {:?} at time {:.1}, excluding from gravity",
                        id,
                        time
                    );
                    result[i] = GravitySourceFull {
                        id,
                        pos: DVec2::ZERO,
                        gm: 0.0,               // Exclude from gravity
                        collision_radius: 0.0, // No collision with missing body
                    };
                }
            }
        }

        result
    }

    /// Check if a position collides with any celestial body.
    ///
    /// Uses `COLLISION_MULTIPLIER` to create a "danger zone" around each body.
    /// This makes collision detection more forgiving for gameplay while representing
    /// realistic planetary defense intervention thresholds.
    ///
    /// # Arguments
    /// * `pos` - Position to check (meters from barycenter)
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Some(CelestialBodyId) if collision detected, None otherwise.
    pub fn check_collision(&self, pos: DVec2, time: f64) -> Option<CelestialBodyId> {
        // Check Sun (use smaller multiplier - Sun is already huge)
        if let Some(sun_data) = self.body_data.get(&CelestialBodyId::Sun)
            && pos.length() < sun_data.radius * 2.0
        {
            return Some(CelestialBodyId::Sun);
        }

        // Check planets - use full COLLISION_MULTIPLIER for danger zone
        // (Moons are decorative only - no collision detection)
        for &id in CelestialBodyId::PLANETS {
            if let (Some(body_pos), Some(data)) =
                (self.get_position_by_id(id, time), self.body_data.get(&id))
                && (pos - body_pos).length() < data.radius * COLLISION_MULTIPLIER
            {
                return Some(id);
            }
        }

        None
    }

    /// Get all registered entity-ID pairs.
    pub fn all_registered(&self) -> impl Iterator<Item = (Entity, CelestialBodyId)> + '_ {
        self.entity_to_id.iter().map(|(&e, &id)| (e, id))
    }

    fn get_kepler_position_by_id(&self, id: CelestialBodyId, time: f64) -> Option<DVec2> {
        let data = self.body_data.get(&id)?;

        match &data.orbit {
            None => Some(DVec2::ZERO), // Sun at origin
            Some(orbit) => {
                let local_pos = orbit.get_local_position(time);

                // Legacy hierarchical orbits (moons): parent heliocentric + local orbit
                match id.parent() {
                    None => Some(local_pos), // Heliocentric orbit
                    Some(parent_id) => {
                        let parent_pos = self.get_position_by_id(parent_id, time)?;
                        Some(parent_pos + local_pos)
                    }
                }
            }
        }
    }

    fn get_kepler_velocity_by_id(&self, id: CelestialBodyId, time: f64) -> Option<DVec2> {
        let data = self.body_data.get(&id)?;

        match &data.orbit {
            None => Some(DVec2::ZERO), // Sun stationary
            Some(orbit) => {
                let local_vel = orbit.get_local_velocity(time);

                // Legacy hierarchical orbits (moons): parent heliocentric + local orbit
                match id.parent() {
                    None => Some(local_vel), // Heliocentric
                    Some(parent_id) => {
                        let parent_vel = self.get_velocity_by_id(parent_id, time)?;
                        Some(parent_vel + local_vel)
                    }
                }
            }
        }
    }

    /// Computes (or reuses) the table→Kepler continuity offset at `t_end`.
    fn get_or_compute_horizons_offset(
        &self,
        id: CelestialBodyId,
        t_end: f64,
    ) -> Option<StateOffset2> {
        // First, try the cache.
        if let Ok(guard) = self.horizons_fallback_offsets.read()
            && let Some(offset) = guard.get(&id).copied()
        {
            return Some(offset);
        }

        let h = self.horizons.as_ref()?;
        let tbl = h.table(id)?;

        let table_end = tbl.sample(t_end).ok()?;
        let kepler_end_pos = self.get_kepler_position_by_id(id, t_end)?;
        let kepler_end_vel = self.get_kepler_velocity_by_id(id, t_end)?;

        let offset = StateOffset2 {
            dp: table_end.pos - kepler_end_pos,
            dv: table_end.vel - kepler_end_vel,
        };

        // Cache it (best-effort; if lock is poisoned, just skip caching).
        if let Ok(mut guard) = self.horizons_fallback_offsets.write() {
            guard.insert(id, offset);
        }

        Some(offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AU_TO_METERS, SECONDS_PER_DAY};

    #[test]
    fn test_ephemeris_creation() {
        let eph = Ephemeris::new();
        assert!(eph.body_data.contains_key(&CelestialBodyId::Sun));
        assert!(eph.body_data.contains_key(&CelestialBodyId::Earth));
        assert!(eph.body_data.contains_key(&CelestialBodyId::Moon));
    }

    #[test]
    fn test_sun_position() {
        let eph = Ephemeris::new();
        let sun_pos = eph.get_position_by_id(CelestialBodyId::Sun, 0.0);
        assert_eq!(sun_pos, Some(DVec2::ZERO));
    }

    #[test]
    fn test_earth_position_at_epoch() {
        let eph = Ephemeris::new();
        let earth_pos = eph.get_position_by_id(CelestialBodyId::Earth, 0.0).unwrap();

        // Earth should be roughly 1 AU from Sun at J2000
        let distance_au = earth_pos.length() / AU_TO_METERS;
        assert!(
            (distance_au - 1.0).abs() < 0.02,
            "Earth should be ~1 AU from Sun, got {} AU",
            distance_au
        );
    }

    #[test]
    fn test_moon_position_relative_to_earth() {
        let eph = Ephemeris::new();

        let earth_pos = eph.get_position_by_id(CelestialBodyId::Earth, 0.0).unwrap();
        let moon_pos = eph.get_position_by_id(CelestialBodyId::Moon, 0.0).unwrap();

        // Moon should be ~384,400 km from Earth
        let distance_km = (moon_pos - earth_pos).length() / 1000.0;
        assert!(
            (distance_km - 384400.0).abs() < 50000.0,
            "Moon should be ~384,400 km from Earth, got {} km",
            distance_km
        );
    }

    #[test]
    fn test_gravity_sources() {
        let eph = Ephemeris::new();
        let sources = eph.get_gravity_sources(0.0);

        // Should have Sun + 8 planets = 9 sources (moons are decorative only)
        assert_eq!(sources.len(), 9);

        // Sun should have largest GM
        let sun_gm = sources[0].1;
        for (_, gm) in &sources[1..] {
            assert!(sun_gm > *gm);
        }
    }

    #[test]
    fn test_collision_detection() {
        let eph = Ephemeris::new();

        // Position at Sun's center should collide
        let collision = eph.check_collision(DVec2::ZERO, 0.0);
        assert_eq!(collision, Some(CelestialBodyId::Sun));

        // Position far from everything should not collide
        let far_pos = DVec2::new(100.0 * AU_TO_METERS, 0.0);
        let no_collision = eph.check_collision(far_pos, 0.0);
        assert!(no_collision.is_none());
    }

    #[test]
    fn test_entity_registration() {
        let mut eph = Ephemeris::new();

        // Create fake entities (in real usage these come from Bevy)
        let fake_entity = Entity::from_raw(42);

        eph.register(fake_entity, CelestialBodyId::Earth);

        assert_eq!(eph.get_id(fake_entity), Some(CelestialBodyId::Earth));
        assert_eq!(eph.get_entity(CelestialBodyId::Earth), Some(fake_entity));
    }

    #[test]
    fn test_planetary_motion_over_time() {
        let eph = Ephemeris::new();

        // Earth position at J2000
        let earth_pos_0 = eph.get_position_by_id(CelestialBodyId::Earth, 0.0).unwrap();

        // Earth position after ~6 months (half orbit)
        let half_year_seconds = 182.625 * SECONDS_PER_DAY;
        let earth_pos_half = eph
            .get_position_by_id(CelestialBodyId::Earth, half_year_seconds)
            .unwrap();

        // Positions should be roughly opposite
        let dot = earth_pos_0.normalize().dot(earth_pos_half.normalize());
        assert!(
            dot < -0.8,
            "Earth after 6 months should be on opposite side of orbit, dot product = {}",
            dot
        );
    }

    #[test]
    fn test_continuity_offsets_do_not_introduce_jumps_when_tables_expire() {
        let eph = Ephemeris::new();

        // If no Horizons tables are loaded (common in CI), this test is a no-op.
        let Some(h) = eph.horizons.as_ref() else {
            return;
        };

        // Pick a representative body that likely has a table if tables are present.
        // (Earth is included in all documented export sets.)
        let id = CelestialBodyId::Earth;
        let Some(cov) = h.coverage(id) else {
            return;
        };

        // Test just beyond the end boundary. We sample both sides of the boundary using a small dt.
        // dt is chosen small enough to approximate continuity while being > 0.
        let dt = 1.0; // 1 second
        let t0 = cov.end;
        let t1 = cov.end + dt;

        let p0 = eph.get_position_by_id(id, t0).unwrap();
        let p1 = eph.get_position_by_id(id, t1).unwrap();
        let v0 = eph.get_velocity_by_id(id, t0).unwrap();
        let v1 = eph.get_velocity_by_id(id, t1).unwrap();

        // Expect that the position advances roughly according to v0 * dt (first-order),
        // and that v is continuous (no huge instantaneous delta).
        let predicted = p0 + v0 * dt;
        let pos_err = (p1 - predicted).length();
        let vel_jump = (v1 - v0).length();

        // These are intentionally loose game-physics tolerances:
        // - position error on the order of km over 1 second would be absurd
        // - velocity jump on the order of km/s would be a visible discontinuity
        assert!(
            pos_err < 1.0e6,
            "Position should remain continuous across table expiry (pos_err = {} m)",
            pos_err
        );
        assert!(
            vel_jump < 1.0e3,
            "Velocity should remain continuous across table expiry (vel_jump = {} m/s)",
            vel_jump
        );
    }

    #[test]
    fn test_continuity_offsets_work_for_moons_if_tables_present() {
        // Note: For moons, we deliberately skip the offset-based continuity approach
        // because the Horizons table data for moons can diverge from the parent planet
        // position over time. Instead, we use pure Kepler (parent + local orbit) outside
        // table coverage, which may create a small discontinuity at the boundary but
        // ensures moons are always correctly positioned near their parent planet.
        //
        // This test now just verifies that moon positions are reasonable on both sides
        // of the table boundary.
        let eph = Ephemeris::new();

        let Some(h) = eph.horizons.as_ref() else {
            return;
        };

        let id = CelestialBodyId::Moon;
        let Some(cov) = h.coverage(id) else {
            return;
        };

        let t0 = cov.end;
        let t1 = cov.end + 1.0;

        let p0 = eph.get_position_by_id(id, t0).unwrap();
        let p1 = eph.get_position_by_id(id, t1).unwrap();

        // Both positions should be at roughly Earth's orbital distance (1 AU ± 0.01 AU for Moon)
        let au = 1.496e11;
        let d0 = p0.length() / au;
        let d1 = p1.length() / au;

        assert!(
            (0.98..=1.02).contains(&d0),
            "Moon at table end should be near 1 AU, got {} AU",
            d0
        );
        assert!(
            (0.98..=1.02).contains(&d1),
            "Moon just past table end should be near 1 AU, got {} AU",
            d1
        );

        // And the Moon should be close to Earth on both sides
        let earth_p0 = eph.get_position_by_id(CelestialBodyId::Earth, t0).unwrap();
        let earth_p1 = eph.get_position_by_id(CelestialBodyId::Earth, t1).unwrap();

        let moon_earth_dist_0 = (p0 - earth_p0).length() / au;
        let moon_earth_dist_1 = (p1 - earth_p1).length() / au;

        // Moon should be within 0.02 AU of Earth (actual distance ~0.0026 AU)
        // Note: The Horizons table data can drift near the end of coverage
        assert!(
            moon_earth_dist_0 < 0.02,
            "Moon at table end should be close to Earth, got {} AU",
            moon_earth_dist_0
        );
        assert!(
            moon_earth_dist_1 < 0.02,
            "Moon past table end should be close to Earth, got {} AU",
            moon_earth_dist_1
        );
    }
}

#[cfg(test)]
mod position_tests {
    use super::*;

    const AU: f64 = 1.496e11;

    /// Tests that moons are correctly positioned near their parent planets.
    /// This tests both within-table-coverage (year 1) and outside-table-coverage (year 26).
    fn test_moons_near_parent(eph: &Ephemeris, time: f64, label: &str) {
        let jupiter_pos = eph
            .get_position_by_id(CelestialBodyId::Jupiter, time)
            .expect("Jupiter position should be available");
        let jupiter_dist_au = jupiter_pos.length() / AU;

        assert!(
            jupiter_dist_au > 4.5 && jupiter_dist_au < 6.5,
            "{}: Jupiter should be 4.5-6.5 AU from Sun, got {:.4} AU",
            label,
            jupiter_dist_au
        );

        for &moon_id in &[
            CelestialBodyId::Io,
            CelestialBodyId::Europa,
            CelestialBodyId::Ganymede,
            CelestialBodyId::Callisto,
        ] {
            let moon_pos = eph
                .get_position_by_id(moon_id, time)
                .expect(&format!("{:?} position should be available", moon_id));
            let dist_from_jupiter = (moon_pos - jupiter_pos).length() / AU;

            assert!(
                dist_from_jupiter < 0.02,
                "{}: {:?} should be < 0.02 AU from Jupiter, got {:.6} AU",
                label,
                moon_id,
                dist_from_jupiter
            );
        }
    }

    #[test]
    fn test_jupiter_moons_within_table_coverage() {
        let eph = Ephemeris::new();
        // 1 year after J2000 - within table coverage
        let time = 365.25 * 86400.0;
        test_moons_near_parent(&eph, time, "year 1");
    }

    #[test]
    fn test_jupiter_moons_outside_table_coverage() {
        let eph = Ephemeris::new();
        // 26 years after J2000 - outside moon table coverage (tables end at ~24 years)
        let time = 26.0 * 365.25 * 86400.0;
        test_moons_near_parent(&eph, time, "year 26");
    }
}
