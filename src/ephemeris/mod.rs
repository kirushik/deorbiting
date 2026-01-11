//! Ephemeris module for computing celestial body positions using Keplerian orbits.

pub mod data;
pub mod kepler;

pub use data::{CelestialBodyData, CelestialBodyId, all_bodies, get_body_data};
pub use kepler::KeplerOrbit;

use bevy::prelude::*;
use bevy::math::DVec2;
use std::collections::HashMap;
use crate::types::G;

/// Resource providing ephemeris data for all celestial bodies.
/// Computes positions analytically using Keplerian orbital elements.
#[derive(Resource)]
pub struct Ephemeris {
    /// Mapping from entity to celestial body ID
    entity_to_id: HashMap<Entity, CelestialBodyId>,
    /// Mapping from celestial body ID to entity
    id_to_entity: HashMap<CelestialBodyId, Entity>,
    /// Cached body data
    body_data: HashMap<CelestialBodyId, CelestialBodyData>,
}

impl Default for Ephemeris {
    fn default() -> Self {
        Self::new()
    }
}

impl Ephemeris {
    /// Create a new ephemeris with all celestial body data loaded.
    pub fn new() -> Self {
        let mut body_data = HashMap::new();
        for data in all_bodies() {
            body_data.insert(data.id, data);
        }

        Self {
            entity_to_id: HashMap::new(),
            id_to_entity: HashMap::new(),
            body_data,
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
        let data = self.body_data.get(&id)?;

        match &data.orbit {
            None => Some(DVec2::ZERO), // Sun at origin
            Some(orbit) => {
                let local_pos = orbit.get_local_position(time);

                // Handle hierarchical orbits (moons)
                match id.parent() {
                    None => Some(local_pos), // Heliocentric orbit
                    Some(parent_id) => {
                        // Moon orbit: add parent planet's position
                        let parent_pos = self.get_position_by_id(parent_id, time)?;
                        Some(parent_pos + local_pos)
                    }
                }
            }
        }
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
        let data = self.body_data.get(&id)?;

        match &data.orbit {
            None => Some(DVec2::ZERO), // Sun stationary
            Some(orbit) => {
                let local_vel = orbit.get_local_velocity(time);

                // Handle hierarchical orbits (moons)
                match id.parent() {
                    None => Some(local_vel), // Heliocentric orbit
                    Some(parent_id) => {
                        // Moon: add parent planet's velocity
                        let parent_vel = self.get_velocity_by_id(parent_id, time)?;
                        Some(parent_vel + local_vel)
                    }
                }
            }
        }
    }

    /// Get all gravity sources at a given time.
    ///
    /// Returns positions and GM (μ = G·M) values, NOT masses.
    /// GM is the standard gravitational parameter in m³/s².
    /// Use directly in acceleration formula: a = GM/r² (no need to multiply by G).
    ///
    /// # Arguments
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Vector of (position in meters, GM in m³/s²) pairs for all massive bodies.
    pub fn get_gravity_sources(&self, time: f64) -> Vec<(DVec2, f64)> {
        let mut sources = Vec::new();

        // Sun
        if let Some(sun_data) = self.body_data.get(&CelestialBodyId::Sun) {
            sources.push((DVec2::ZERO, G * sun_data.mass));
        }

        // Planets
        for &id in CelestialBodyId::PLANETS {
            if let (Some(pos), Some(data)) = (
                self.get_position_by_id(id, time),
                self.body_data.get(&id),
            ) {
                sources.push((pos, G * data.mass));
            }
        }

        // Moons (significant for close encounters)
        for &id in CelestialBodyId::MOONS {
            if let (Some(pos), Some(data)) = (
                self.get_position_by_id(id, time),
                self.body_data.get(&id),
            ) {
                sources.push((pos, G * data.mass));
            }
        }

        sources
    }

    /// Check if a position collides with any celestial body.
    ///
    /// # Arguments
    /// * `pos` - Position to check (meters from barycenter)
    /// * `time` - Time in seconds since J2000 epoch
    ///
    /// # Returns
    /// Some(CelestialBodyId) if collision detected, None otherwise.
    pub fn check_collision(&self, pos: DVec2, time: f64) -> Option<CelestialBodyId> {
        // Check Sun
        if let Some(sun_data) = self.body_data.get(&CelestialBodyId::Sun) {
            if pos.length() < sun_data.radius {
                return Some(CelestialBodyId::Sun);
            }
        }

        // Check planets
        for &id in CelestialBodyId::PLANETS {
            if let (Some(body_pos), Some(data)) = (
                self.get_position_by_id(id, time),
                self.body_data.get(&id),
            ) {
                if (pos - body_pos).length() < data.radius {
                    return Some(id);
                }
            }
        }

        // Check moons
        for &id in CelestialBodyId::MOONS {
            if let (Some(body_pos), Some(data)) = (
                self.get_position_by_id(id, time),
                self.body_data.get(&id),
            ) {
                if (pos - body_pos).length() < data.radius {
                    return Some(id);
                }
            }
        }

        None
    }

    /// Get all registered entity-ID pairs.
    pub fn all_registered(&self) -> impl Iterator<Item = (Entity, CelestialBodyId)> + '_ {
        self.entity_to_id.iter().map(|(&e, &id)| (e, id))
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

        // Should have Sun + 8 planets + 6 moons = 15 sources
        assert_eq!(sources.len(), 15);

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
}
