//! Deflection payload types and delta-v calculations.
//!
//! Implements physics for:
//! - Kinetic impactor (DART-style): momentum transfer with ejecta amplification
//! - Nuclear standoff: vaporization impulse

use bevy::math::DVec2;

/// Deflection payload configuration.
#[derive(Clone, Debug)]
pub enum DeflectionPayload {
    /// Kinetic impactor - transfers momentum through collision.
    ///
    /// Delta-v = β × (m × v_rel) / M_asteroid
    /// where β is the momentum enhancement factor from ejecta.
    Kinetic {
        /// Impactor mass (kg). Typical: 500-1000 kg.
        mass_kg: f64,
        /// Momentum enhancement factor (dimensionless).
        /// DART measured β ≈ 3.6 for Dimorphos.
        /// Range: 1.0 (no ejecta) to 5.0+ (significant ejecta).
        beta: f64,
    },

    /// Nuclear standoff detonation - vaporizes surface material.
    ///
    /// Based on LLNL research: ~2 cm/s per 100 kt for a 300m asteroid.
    /// Scales inversely with asteroid mass.
    Nuclear {
        /// Weapon yield (kilotons TNT equivalent).
        /// Range: 1-1000 kt.
        yield_kt: f64,
    },
}

impl Default for DeflectionPayload {
    fn default() -> Self {
        // DART-like default
        Self::Kinetic {
            mass_kg: 560.0,
            beta: 3.6,
        }
    }
}

impl DeflectionPayload {
    /// Create a DART-like kinetic impactor.
    pub fn dart() -> Self {
        Self::Kinetic {
            mass_kg: 560.0,
            beta: 3.6,
        }
    }

    /// Create a heavy kinetic impactor.
    pub fn heavy_kinetic() -> Self {
        Self::Kinetic {
            mass_kg: 1000.0,
            beta: 3.0,
        }
    }

    /// Create a nuclear standoff device.
    pub fn nuclear(yield_kt: f64) -> Self {
        Self::Nuclear { yield_kt }
    }

    /// Calculate the delta-v imparted to the asteroid.
    ///
    /// # Arguments
    /// * `asteroid_mass` - Mass of the asteroid (kg)
    /// * `relative_velocity` - Relative velocity of impact (m/s) - for kinetic only
    /// * `direction` - Unit vector in the desired deflection direction
    ///
    /// # Returns
    /// Delta-v vector (m/s) applied to the asteroid
    pub fn calculate_delta_v(
        &self,
        asteroid_mass: f64,
        relative_velocity: f64,
        direction: DVec2,
    ) -> DVec2 {
        let delta_v_magnitude = match self {
            DeflectionPayload::Kinetic { mass_kg, beta } => {
                // DART formula: Δv = β × (m × v_rel) / M
                // This accounts for momentum enhancement from ejecta
                beta * mass_kg * relative_velocity / asteroid_mass
            }

            DeflectionPayload::Nuclear { yield_kt } => {
                // Based on LLNL research (Wie et al.):
                // ~2 cm/s per 100 kt for a 300m asteroid (~3×10^10 kg)
                // Scale: Δv ∝ yield / mass
                //
                // Reference point: 100 kt → 0.02 m/s for 3e10 kg
                // So: Δv = 0.02 * (yield_kt / 100) * (3e10 / asteroid_mass)
                let reference_delta_v = 0.02; // m/s
                let reference_yield = 100.0; // kt
                let reference_mass = 3e10; // kg (300m rocky asteroid)

                reference_delta_v * (yield_kt / reference_yield) * (reference_mass / asteroid_mass)
            }
        };

        direction.normalize_or_zero() * delta_v_magnitude
    }

    /// Get a human-readable description of the payload.
    pub fn description(&self) -> String {
        match self {
            DeflectionPayload::Kinetic { mass_kg, beta } => {
                format!("Kinetic Impactor ({:.0} kg, β={:.1})", mass_kg, beta)
            }
            DeflectionPayload::Nuclear { yield_kt } => {
                if *yield_kt >= 1000.0 {
                    format!("Nuclear Standoff ({:.1} Mt)", yield_kt / 1000.0)
                } else {
                    format!("Nuclear Standoff ({:.0} kt)", yield_kt)
                }
            }
        }
    }

    /// Estimate the delta-v for display (using typical values).
    ///
    /// Uses 6 km/s relative velocity for kinetic (typical Earth-crossing approach).
    pub fn estimate_delta_v(&self, asteroid_mass: f64) -> f64 {
        let typical_relative_velocity = 6000.0; // 6 km/s
        self.calculate_delta_v(asteroid_mass, typical_relative_velocity, DVec2::X)
            .length()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dart_like_kinetic() {
        // DART mission parameters (approximate):
        // - 560 kg spacecraft
        // - 6.1 km/s relative velocity
        // - β ≈ 3.6 (measured)
        // - Dimorphos mass ~4.3×10^9 kg
        // - Measured Δv ≈ 2.7 mm/s

        let payload = DeflectionPayload::dart();
        let dimorphos_mass = 4.3e9; // kg
        let relative_velocity = 6100.0; // m/s
        let direction = DVec2::X;

        let delta_v = payload.calculate_delta_v(dimorphos_mass, relative_velocity, direction);

        // Expected: β × m × v / M = 3.6 × 560 × 6100 / 4.3e9 ≈ 2.86 mm/s
        let expected = 3.6 * 560.0 * 6100.0 / dimorphos_mass;
        let error = (delta_v.length() - expected).abs() / expected;

        assert!(
            error < 1e-10,
            "DART calculation error: {error}, got {} mm/s, expected {} mm/s",
            delta_v.length() * 1000.0,
            expected * 1000.0
        );

        // Should be close to real DART result (~2.7 mm/s)
        assert!(
            (delta_v.length() * 1000.0 - 2.86).abs() < 0.1,
            "Should be ~2.86 mm/s, got {} mm/s",
            delta_v.length() * 1000.0
        );
    }

    #[test]
    fn test_nuclear_reference() {
        // Reference: 100 kt → 2 cm/s for 300m asteroid (3×10^10 kg)
        let payload = DeflectionPayload::nuclear(100.0);
        let asteroid_mass = 3e10;
        let direction = DVec2::X;

        let delta_v = payload.calculate_delta_v(asteroid_mass, 0.0, direction);

        assert!(
            (delta_v.length() - 0.02).abs() < 1e-10,
            "100 kt should give 2 cm/s for 3e10 kg asteroid, got {} m/s",
            delta_v.length()
        );
    }

    #[test]
    fn test_nuclear_scaling() {
        // Doubling yield should double delta-v
        let payload_100 = DeflectionPayload::nuclear(100.0);
        let payload_200 = DeflectionPayload::nuclear(200.0);
        let asteroid_mass = 3e10;
        let direction = DVec2::X;

        let dv_100 = payload_100.calculate_delta_v(asteroid_mass, 0.0, direction);
        let dv_200 = payload_200.calculate_delta_v(asteroid_mass, 0.0, direction);

        let ratio = dv_200.length() / dv_100.length();
        assert!(
            (ratio - 2.0).abs() < 1e-10,
            "Doubling yield should double delta-v, ratio = {ratio}"
        );
    }

    #[test]
    fn test_nuclear_mass_scaling() {
        // Halving asteroid mass should double delta-v
        let payload = DeflectionPayload::nuclear(100.0);
        let direction = DVec2::X;

        let dv_large = payload.calculate_delta_v(3e10, 0.0, direction);
        let dv_small = payload.calculate_delta_v(1.5e10, 0.0, direction);

        let ratio = dv_small.length() / dv_large.length();
        assert!(
            (ratio - 2.0).abs() < 1e-10,
            "Half mass should double delta-v, ratio = {ratio}"
        );
    }

    #[test]
    fn test_direction_preserved() {
        let payload = DeflectionPayload::dart();
        let asteroid_mass = 1e10;
        let velocity = 6000.0;

        // Test various directions
        for angle in [0.0_f64, 45.0, 90.0, 180.0, 270.0] {
            let rad = angle.to_radians();
            let direction = DVec2::new(rad.cos(), rad.sin());
            let delta_v = payload.calculate_delta_v(asteroid_mass, velocity, direction);

            // Direction should be preserved
            let result_dir = delta_v.normalize();
            let dot = result_dir.dot(direction);
            assert!(
                (dot - 1.0).abs() < 1e-10,
                "Direction not preserved for angle {angle}°"
            );
        }
    }

    #[test]
    fn test_description() {
        let kinetic = DeflectionPayload::dart();
        assert!(kinetic.description().contains("Kinetic"));
        assert!(kinetic.description().contains("560"));

        let nuclear = DeflectionPayload::nuclear(500.0);
        assert!(nuclear.description().contains("Nuclear"));
        assert!(nuclear.description().contains("500 kt"));

        let big_nuclear = DeflectionPayload::nuclear(2000.0);
        assert!(big_nuclear.description().contains("Mt"));
    }
}
