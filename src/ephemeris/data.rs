//! Orbital elements data for solar system bodies (J2000 epoch).
//! Source: NASA JPL Horizons, simplified for 2D ecliptic plane.

use super::kepler::KeplerOrbit;
use crate::types::AU_TO_METERS;

/// Identifier for celestial bodies in the simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CelestialBodyId {
    Sun,
    Mercury,
    Venus,
    Earth,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    // Moons
    Moon,
    Io,
    Europa,
    Ganymede,
    Callisto,
    Titan,
}

impl CelestialBodyId {
    /// All planets (not including Sun or moons)
    pub const PLANETS: &'static [CelestialBodyId] = &[
        CelestialBodyId::Mercury,
        CelestialBodyId::Venus,
        CelestialBodyId::Earth,
        CelestialBodyId::Mars,
        CelestialBodyId::Jupiter,
        CelestialBodyId::Saturn,
        CelestialBodyId::Uranus,
        CelestialBodyId::Neptune,
    ];

    /// All moons
    pub const MOONS: &'static [CelestialBodyId] = &[
        CelestialBodyId::Moon,
        CelestialBodyId::Io,
        CelestialBodyId::Europa,
        CelestialBodyId::Ganymede,
        CelestialBodyId::Callisto,
        CelestialBodyId::Titan,
    ];

    /// Get the parent body (for moons)
    pub fn parent(&self) -> Option<CelestialBodyId> {
        match self {
            CelestialBodyId::Moon => Some(CelestialBodyId::Earth),
            CelestialBodyId::Io
            | CelestialBodyId::Europa
            | CelestialBodyId::Ganymede
            | CelestialBodyId::Callisto => Some(CelestialBodyId::Jupiter),
            CelestialBodyId::Titan => Some(CelestialBodyId::Saturn),
            _ => None,
        }
    }

    /// Human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            CelestialBodyId::Sun => "Sun",
            CelestialBodyId::Mercury => "Mercury",
            CelestialBodyId::Venus => "Venus",
            CelestialBodyId::Earth => "Earth",
            CelestialBodyId::Mars => "Mars",
            CelestialBodyId::Jupiter => "Jupiter",
            CelestialBodyId::Saturn => "Saturn",
            CelestialBodyId::Uranus => "Uranus",
            CelestialBodyId::Neptune => "Neptune",
            CelestialBodyId::Moon => "Moon",
            CelestialBodyId::Io => "Io",
            CelestialBodyId::Europa => "Europa",
            CelestialBodyId::Ganymede => "Ganymede",
            CelestialBodyId::Callisto => "Callisto",
            CelestialBodyId::Titan => "Titan",
        }
    }
}

/// Static data for a celestial body.
// TODO: When implementing visualization (Phase 1+), move `visual_scale` to a separate
// render-specific struct. Physics data and render data should be decoupled.
#[derive(Clone, Debug)]
pub struct CelestialBodyData {
    pub id: CelestialBodyId,
    pub mass: f64,   // kg
    pub radius: f64, // meters
    pub orbit: Option<KeplerOrbit>,
    pub visual_scale: f32, // Multiplier for rendering size (temporary, see TODO above)
}

impl CelestialBodyData {
    /// Get mass in kg
    pub fn mass(&self) -> f64 {
        self.mass
    }

    /// Get physical radius in meters
    pub fn radius(&self) -> f64 {
        self.radius
    }
}

/// Get orbital and physical data for a celestial body.
pub fn get_body_data(id: CelestialBodyId) -> CelestialBodyData {
    match id {
        CelestialBodyId::Sun => CelestialBodyData {
            id,
            mass: 1.989e30,
            radius: 6.963e8,
            orbit: None, // Sun is at origin
            visual_scale: 20.0,
        },

        // Planets (heliocentric orbits)
        CelestialBodyId::Mercury => CelestialBodyData {
            id,
            mass: 3.302e23,
            radius: 2.440e6,
            orbit: Some(KeplerOrbit::from_elements(
                0.387 * AU_TO_METERS, // semi-major axis
                0.2056,               // eccentricity
                29.12,                // argument of periapsis (deg)
                174.79,               // mean anomaly at epoch (deg)
                4.0923,               // mean motion (deg/day)
            )),
            visual_scale: 200.0,
        },

        CelestialBodyId::Venus => CelestialBodyData {
            id,
            mass: 4.869e24,
            radius: 6.052e6,
            orbit: Some(KeplerOrbit::from_elements(
                0.723 * AU_TO_METERS,
                0.0068,
                54.85,
                50.42,
                1.6021,
            )),
            visual_scale: 150.0,
        },

        CelestialBodyId::Earth => CelestialBodyData {
            id,
            mass: 5.972e24,
            radius: 6.371e6,
            orbit: Some(KeplerOrbit::from_elements(
                1.000 * AU_TO_METERS,
                0.0167,
                102.94,
                357.53,
                0.9856,
            )),
            visual_scale: 150.0,
        },

        CelestialBodyId::Mars => CelestialBodyData {
            id,
            mass: 6.417e23,
            radius: 3.390e6,
            orbit: Some(KeplerOrbit::from_elements(
                1.524 * AU_TO_METERS,
                0.0934,
                286.50,
                19.41,
                0.5240,
            )),
            visual_scale: 180.0,
        },

        CelestialBodyId::Jupiter => CelestialBodyData {
            id,
            mass: 1.898e27,
            radius: 6.991e7,
            orbit: Some(KeplerOrbit::from_elements(
                5.203 * AU_TO_METERS,
                0.0484,
                273.87,
                20.02,
                0.0831,
            )),
            visual_scale: 50.0,
        },

        CelestialBodyId::Saturn => CelestialBodyData {
            id,
            mass: 5.683e26,
            radius: 5.823e7,
            orbit: Some(KeplerOrbit::from_elements(
                9.537 * AU_TO_METERS,
                0.0542,
                339.39,
                317.02,
                0.0335,
            )),
            visual_scale: 55.0,
        },

        CelestialBodyId::Uranus => CelestialBodyData {
            id,
            mass: 8.681e25,
            radius: 2.536e7,
            orbit: Some(KeplerOrbit::from_elements(
                19.19 * AU_TO_METERS,
                0.0472,
                96.99,
                142.24,
                0.0117,
            )),
            visual_scale: 80.0,
        },

        CelestialBodyId::Neptune => CelestialBodyData {
            id,
            mass: 1.024e26,
            radius: 2.462e7,
            orbit: Some(KeplerOrbit::from_elements(
                30.07 * AU_TO_METERS,
                0.0086,
                273.19,
                256.23,
                0.0060,
            )),
            visual_scale: 80.0,
        },

        // Moons (parent-relative orbits)
        CelestialBodyId::Moon => CelestialBodyData {
            id,
            mass: 7.342e22,
            radius: 1.737e6,
            orbit: Some(KeplerOrbit::from_elements(
                3.844e8, // 384,400 km
                0.0549, 318.15,  // argument of periapsis
                134.96,  // mean anomaly at J2000
                13.1764, // mean motion (deg/day) - 27.32 day period
            )),
            visual_scale: 250.0,
        },

        CelestialBodyId::Io => CelestialBodyData {
            id,
            mass: 8.932e22,
            radius: 1.822e6,
            orbit: Some(KeplerOrbit::from_elements(
                4.218e8, // 421,800 km
                0.0041, 84.13, 342.02, 203.49, // 1.77 day period -> deg/day
            )),
            visual_scale: 300.0,
        },

        CelestialBodyId::Europa => CelestialBodyData {
            id,
            mass: 4.800e22,
            radius: 1.561e6,
            orbit: Some(KeplerOrbit::from_elements(
                6.711e8, // 671,100 km
                0.0094, 88.97, 171.02, 101.37, // 3.55 day period
            )),
            visual_scale: 300.0,
        },

        CelestialBodyId::Ganymede => CelestialBodyData {
            id,
            mass: 1.482e23,
            radius: 2.634e6,
            orbit: Some(KeplerOrbit::from_elements(
                1.070e9, // 1,070,400 km
                0.0011, 192.42, 317.54, 50.32, // 7.15 day period
            )),
            visual_scale: 280.0,
        },

        CelestialBodyId::Callisto => CelestialBodyData {
            id,
            mass: 1.076e23,
            radius: 2.410e6,
            orbit: Some(KeplerOrbit::from_elements(
                1.883e9, // 1,882,700 km
                0.0074, 52.64, 181.41, 21.57, // 16.69 day period
            )),
            visual_scale: 280.0,
        },

        CelestialBodyId::Titan => CelestialBodyData {
            id,
            mass: 1.345e23,
            radius: 2.575e6,
            orbit: Some(KeplerOrbit::from_elements(
                1.222e9, // 1,221,870 km
                0.0288, 180.53, 163.31, 22.58, // 15.95 day period
            )),
            visual_scale: 280.0,
        },
    }
}

/// Get data for all celestial bodies.
pub fn all_bodies() -> Vec<CelestialBodyData> {
    let mut bodies = vec![get_body_data(CelestialBodyId::Sun)];

    for &id in CelestialBodyId::PLANETS {
        bodies.push(get_body_data(id));
    }

    for &id in CelestialBodyId::MOONS {
        bodies.push(get_body_data(id));
    }

    bodies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_bodies_have_data() {
        let bodies = all_bodies();
        // Sun + 8 planets + 6 moons = 15
        assert_eq!(bodies.len(), 15);
    }

    #[test]
    fn test_sun_has_no_orbit() {
        let sun = get_body_data(CelestialBodyId::Sun);
        assert!(sun.orbit.is_none());
    }

    #[test]
    fn test_planets_have_orbits() {
        for &id in CelestialBodyId::PLANETS {
            let data = get_body_data(id);
            assert!(data.orbit.is_some(), "{} should have an orbit", id.name());
        }
    }

    #[test]
    fn test_moon_parents() {
        assert_eq!(CelestialBodyId::Moon.parent(), Some(CelestialBodyId::Earth));
        assert_eq!(CelestialBodyId::Io.parent(), Some(CelestialBodyId::Jupiter));
        assert_eq!(
            CelestialBodyId::Titan.parent(),
            Some(CelestialBodyId::Saturn)
        );
        assert_eq!(CelestialBodyId::Earth.parent(), None);
    }

    #[test]
    fn test_body_masses_reasonable() {
        // Sun should be most massive
        let sun = get_body_data(CelestialBodyId::Sun);
        let jupiter = get_body_data(CelestialBodyId::Jupiter);
        let earth = get_body_data(CelestialBodyId::Earth);

        assert!(sun.mass > jupiter.mass);
        assert!(jupiter.mass > earth.mass);
    }

    #[test]
    fn test_body_radii_reasonable() {
        let sun = get_body_data(CelestialBodyId::Sun);
        let jupiter = get_body_data(CelestialBodyId::Jupiter);
        let earth = get_body_data(CelestialBodyId::Earth);
        let moon = get_body_data(CelestialBodyId::Moon);

        assert!(sun.radius > jupiter.radius);
        assert!(jupiter.radius > earth.radius);
        assert!(earth.radius > moon.radius);
    }
}
