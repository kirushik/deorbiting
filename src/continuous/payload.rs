//! Continuous deflection payload types.
//!
//! These methods apply small forces over extended periods, requiring
//! integration into the physics loop rather than instant delta-v application.

use super::thrust::ThrustDirection;

/// Continuous deflection payload configuration.
///
/// Unlike instant deflection methods (kinetic/nuclear), these apply
/// continuous thrust over time and need to be integrated into the physics loop.
#[derive(Clone, Debug)]
pub enum ContinuousPayload {
    /// Ion Beam Shepherd - spacecraft hovers near asteroid, ion exhaust pushes it.
    ///
    /// Most efficient for asteroids < 2 km diameter.
    /// Typical mission duration: months to years.
    IonBeam {
        /// Ion engine thrust in Newtons (typically 10 mN - 1 N).
        thrust_n: f64,
        /// Propellant mass in kg. Mission ends when depleted.
        fuel_mass_kg: f64,
        /// Specific impulse in seconds (typically 3000-5000 s).
        specific_impulse: f64,
        /// Hover distance from asteroid surface in meters (50-500 m).
        hover_distance_m: f64,
        /// Thrust direction relative to asteroid velocity.
        direction: ThrustDirection,
    },

    /// Gravity Tractor - spacecraft mass gravitationally pulls asteroid.
    ///
    /// Slowest method but most precise and predictable.
    /// Requires decades of lead time for meaningful deflection.
    GravityTractor {
        /// Spacecraft mass in kg (typical: 10,000-20,000 kg).
        spacecraft_mass_kg: f64,
        /// Hover distance from asteroid center in meters (150-300 m).
        hover_distance_m: f64,
        /// Mission duration in seconds. Operation is passive (no fuel consumption).
        mission_duration: f64,
        /// Direction of gravitational pull (spacecraft positioning).
        direction: ThrustDirection,
    },

    /// Laser Ablation - vaporizes asteroid surface, creating thrust plume.
    ///
    /// Effectiveness varies with solar distance (solar-powered).
    /// Based on DE-STARLITE concept.
    LaserAblation {
        /// Laser power in kilowatts (typical: 50-1000 kW).
        power_kw: f64,
        /// Mission duration in seconds.
        mission_duration: f64,
        /// Laser efficiency factor (0.0-1.0, accounts for losses).
        efficiency: f64,
        /// Direction to point the laser (affects thrust direction on asteroid).
        direction: ThrustDirection,
    },
}

impl Default for ContinuousPayload {
    fn default() -> Self {
        ContinuousPayload::ion_beam_default()
    }
}

impl ContinuousPayload {
    /// Default ion beam shepherd configuration.
    ///
    /// Based on typical mission parameters for ~300m asteroid deflection.
    pub fn ion_beam_default() -> Self {
        ContinuousPayload::IonBeam {
            thrust_n: 0.1,                // 100 mN
            fuel_mass_kg: 500.0,          // 500 kg propellant
            specific_impulse: 3500.0,     // Typical xenon ion engine
            hover_distance_m: 200.0,      // 200 m from surface
            direction: ThrustDirection::Retrograde,
        }
    }

    /// Default gravity tractor configuration.
    ///
    /// Based on Lu & Love (2005) reference mission.
    pub fn gravity_tractor_default() -> Self {
        ContinuousPayload::GravityTractor {
            spacecraft_mass_kg: 20_000.0, // 20 tons
            hover_distance_m: 200.0,      // 200 m from center
            mission_duration: 10.0 * 365.25 * 86400.0, // 10 years
            direction: ThrustDirection::Retrograde,
        }
    }

    /// Default laser ablation configuration.
    ///
    /// Based on DE-STARLITE concept for Apophis-class asteroid.
    pub fn laser_ablation_default() -> Self {
        ContinuousPayload::LaserAblation {
            power_kw: 100.0,              // 100 kW laser
            mission_duration: 1.0 * 365.25 * 86400.0, // 1 year
            efficiency: 0.8,              // 80% efficiency
            direction: ThrustDirection::Retrograde,
        }
    }

    /// Get the thrust direction for this payload.
    pub fn direction(&self) -> ThrustDirection {
        match self {
            ContinuousPayload::IonBeam { direction, .. } => *direction,
            ContinuousPayload::GravityTractor { direction, .. } => *direction,
            ContinuousPayload::LaserAblation { direction, .. } => *direction,
        }
    }

    /// Get a description of this payload type.
    pub fn description(&self) -> &'static str {
        match self {
            ContinuousPayload::IonBeam { .. } => {
                "Ion Beam Shepherd: Spacecraft hovers near asteroid, directing ion exhaust at surface"
            }
            ContinuousPayload::GravityTractor { .. } => {
                "Gravity Tractor: Spacecraft mass gravitationally pulls asteroid over time"
            }
            ContinuousPayload::LaserAblation { .. } => {
                "Laser Ablation: High-power laser vaporizes surface, creating thrust plume"
            }
        }
    }

    /// Get a short name for this payload type.
    pub fn name(&self) -> &'static str {
        match self {
            ContinuousPayload::IonBeam { .. } => "Ion Beam",
            ContinuousPayload::GravityTractor { .. } => "Gravity Tractor",
            ContinuousPayload::LaserAblation { .. } => "Laser Ablation",
        }
    }

    /// Check if this payload consumes fuel.
    pub fn uses_fuel(&self) -> bool {
        matches!(self, ContinuousPayload::IonBeam { .. })
    }

    /// Get initial fuel mass if applicable.
    pub fn initial_fuel(&self) -> Option<f64> {
        match self {
            ContinuousPayload::IonBeam { fuel_mass_kg, .. } => Some(*fuel_mass_kg),
            _ => None,
        }
    }

    /// Get mission duration (for time-limited methods).
    pub fn mission_duration(&self) -> Option<f64> {
        match self {
            ContinuousPayload::IonBeam { .. } => None, // Limited by fuel, not time
            ContinuousPayload::GravityTractor { mission_duration, .. } => Some(*mission_duration),
            ContinuousPayload::LaserAblation { mission_duration, .. } => Some(*mission_duration),
        }
    }

    /// Estimate total delta-v this payload can deliver to an asteroid.
    ///
    /// # Arguments
    /// * `asteroid_mass_kg` - Mass of target asteroid
    /// * `solar_distance_au` - Distance from Sun (for laser ablation)
    ///
    /// # Returns
    /// Estimated total delta-v in m/s
    pub fn estimate_total_delta_v(&self, asteroid_mass_kg: f64, solar_distance_au: f64) -> f64 {
        use super::thrust::{
            gravity_tractor_acceleration, ion_beam_acceleration, ion_fuel_consumption_rate,
            laser_ablation_acceleration,
        };

        match self {
            ContinuousPayload::IonBeam {
                thrust_n,
                fuel_mass_kg,
                specific_impulse,
                ..
            } => {
                let acc = ion_beam_acceleration(*thrust_n, asteroid_mass_kg);
                let mdot = ion_fuel_consumption_rate(*thrust_n, *specific_impulse);
                if mdot > 0.0 {
                    let burn_time = fuel_mass_kg / mdot;
                    acc * burn_time
                } else {
                    0.0
                }
            }
            ContinuousPayload::GravityTractor {
                spacecraft_mass_kg,
                hover_distance_m,
                mission_duration,
                ..
            } => {
                let acc = gravity_tractor_acceleration(*spacecraft_mass_kg, *hover_distance_m);
                acc * mission_duration
            }
            ContinuousPayload::LaserAblation {
                power_kw,
                mission_duration,
                efficiency,
                ..
            } => {
                let acc = laser_ablation_acceleration(
                    power_kw * efficiency,
                    solar_distance_au,
                    asteroid_mass_kg,
                );
                acc * mission_duration
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ion_beam_default() {
        let payload = ContinuousPayload::ion_beam_default();
        match payload {
            ContinuousPayload::IonBeam {
                thrust_n,
                fuel_mass_kg,
                ..
            } => {
                assert!((thrust_n - 0.1).abs() < 1e-10);
                assert!((fuel_mass_kg - 500.0).abs() < 1e-10);
            }
            _ => panic!("Expected IonBeam"),
        }
    }

    #[test]
    fn test_gravity_tractor_default() {
        let payload = ContinuousPayload::gravity_tractor_default();
        match payload {
            ContinuousPayload::GravityTractor {
                spacecraft_mass_kg,
                hover_distance_m,
                ..
            } => {
                assert!((spacecraft_mass_kg - 20_000.0).abs() < 1e-10);
                assert!((hover_distance_m - 200.0).abs() < 1e-10);
            }
            _ => panic!("Expected GravityTractor"),
        }
    }

    #[test]
    fn test_laser_ablation_default() {
        let payload = ContinuousPayload::laser_ablation_default();
        match payload {
            ContinuousPayload::LaserAblation { power_kw, .. } => {
                assert!((power_kw - 100.0).abs() < 1e-10);
            }
            _ => panic!("Expected LaserAblation"),
        }
    }

    #[test]
    fn test_payload_names() {
        assert_eq!(ContinuousPayload::ion_beam_default().name(), "Ion Beam");
        assert_eq!(
            ContinuousPayload::gravity_tractor_default().name(),
            "Gravity Tractor"
        );
        assert_eq!(
            ContinuousPayload::laser_ablation_default().name(),
            "Laser Ablation"
        );
    }

    #[test]
    fn test_uses_fuel() {
        assert!(ContinuousPayload::ion_beam_default().uses_fuel());
        assert!(!ContinuousPayload::gravity_tractor_default().uses_fuel());
        assert!(!ContinuousPayload::laser_ablation_default().uses_fuel());
    }

    #[test]
    fn test_estimate_delta_v_ion_beam() {
        let payload = ContinuousPayload::ion_beam_default();
        let asteroid_mass = 1e10; // 10 billion kg
        let delta_v = payload.estimate_total_delta_v(asteroid_mass, 1.0);
        // Should be positive and reasonable
        assert!(delta_v > 0.0);
        assert!(delta_v < 1.0); // Less than 1 m/s for this size asteroid
    }

    #[test]
    fn test_estimate_delta_v_gravity_tractor() {
        let payload = ContinuousPayload::gravity_tractor_default();
        let asteroid_mass = 1e10;
        let delta_v = payload.estimate_total_delta_v(asteroid_mass, 1.0);
        // Gravity tractor is very slow
        assert!(delta_v > 0.0);
        assert!(delta_v < 0.1); // Very small delta-v even over 10 years
    }
}
