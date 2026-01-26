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

    /// Nuclear deep penetration - splits asteroid into fragments.
    ///
    /// "Armageddon" style: detonation inside asteroid breaks it apart.
    /// Creates two fragments with diverging trajectories.
    /// WARNING: May create multiple collision threats!
    NuclearSplit {
        /// Weapon yield (kilotons TNT equivalent).
        yield_kt: f64,
        /// Mass ratio for the split (fraction for first fragment, 0.0-1.0).
        /// 0.5 = equal split, 0.7 = 70/30 split, etc.
        split_ratio: f64,
    },
}

impl Default for DeflectionPayload {
    fn default() -> Self {
        // Heavy kinetic impactor - effective at gameplay timescales
        Self::Kinetic {
            mass_kg: 10_000.0, // 10 tons (inflated from DART's 560 kg)
            beta: 5.0,         // Higher momentum enhancement
        }
    }
}

impl DeflectionPayload {
    /// Create a heavy kinetic impactor (effective for gameplay).
    ///
    /// Default parameters are significantly inflated from real DART mission values
    /// to be effective at gameplay timescales (days to months).
    ///
    /// Against a 300m asteroid (3e10 kg) at 30 km/s relative velocity:
    /// - Provides ~8 m/s delta-v per impact
    /// - 4 impacts = ~32 m/s (effective for intercepts at 0.1 AU / 6 days out)
    pub fn dart() -> Self {
        Self::Kinetic {
            mass_kg: 200_000.0, // 200 tons (gameplay-scaled from real 560 kg)
            beta: 40.0,         // High momentum enhancement for gameplay
        }
    }

    /// Create an extra-heavy kinetic impactor.
    pub fn heavy_kinetic() -> Self {
        Self::Kinetic {
            mass_kg: 250_000.0, // 250 tons (gameplay-scaled)
            beta: 20.0,         // High momentum enhancement for gameplay
        }
    }

    /// Create a nuclear standoff device.
    pub fn nuclear(yield_kt: f64) -> Self {
        Self::Nuclear { yield_kt }
    }

    /// Create a nuclear splitting device.
    pub fn nuclear_split(yield_kt: f64, split_ratio: f64) -> Self {
        Self::NuclearSplit {
            yield_kt,
            split_ratio: split_ratio.clamp(0.1, 0.9),
        }
    }

    /// Create a nuclear standoff device with gameplay-balanced defaults.
    ///
    /// Returns a 12 megaton device (inflated from realistic values for gameplay).
    /// Against a 300m asteroid (3e10 kg):
    /// - Provides ~36 m/s delta-v per detonation
    /// - Single use effective at 0.1 AU (6 days before impact)
    /// - 2 detonations = ~72 m/s (effective at 0.05 AU / 3 days out)
    pub fn nuclear_default() -> Self {
        Self::Nuclear { yield_kt: 12_000.0 }
    }

    /// Create a nuclear splitting device with gameplay-balanced defaults.
    ///
    /// Returns a 50 megaton device with 50/50 split ratio.
    /// Creates two fragments with diverging trajectories - use with caution
    /// as both fragments may still pose a threat!
    /// Combined with 10% energy efficiency, yields ~37 m/s separation velocity.
    pub fn nuclear_split_default() -> Self {
        Self::NuclearSplit {
            yield_kt: 50_000.0, // 50 MT for dramatic separation
            split_ratio: 0.5,
        }
    }

    /// Check if this payload splits the asteroid instead of deflecting it.
    pub fn is_splitting(&self) -> bool {
        matches!(self, DeflectionPayload::NuclearSplit { .. })
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
                // Inspired by LLNL research (Wie et al.), boosted ~15× for gameplay.
                // Real physics: ~2 cm/s per 100 kt for a 300m asteroid.
                // Gameplay: 30 cm/s reference for meaningful deflection within days.
                //
                // Reference point: 100 kt → 0.30 m/s for 3e10 kg
                // So: Δv = 0.30 * (yield_kt / 100) * (3e10 / asteroid_mass)
                let reference_delta_v = 0.30; // m/s (~15× boost for gameplay)
                let reference_yield = 100.0; // kt
                let reference_mass = 3e10; // kg (300m rocky asteroid)

                reference_delta_v * (yield_kt / reference_yield) * (reference_mass / asteroid_mass)
            }

            DeflectionPayload::NuclearSplit { .. } => {
                // Splitting payloads don't apply delta-v to the original asteroid.
                // They destroy it and create fragments with separation velocity.
                0.0
            }
        };

        direction.normalize_or_zero() * delta_v_magnitude
    }

    /// Calculate the separation velocity for asteroid fragments.
    ///
    /// Based on nuclear detonation energy transfer to fragment kinetic energy.
    /// Using ~2% efficiency for deep-buried detonation (boosted from ~1% for dramatic effect).
    ///
    /// # Arguments
    /// * `yield_kt` - Yield in kilotons
    /// * `total_mass` - Total asteroid mass in kg
    ///
    /// # Returns
    /// Separation velocity in m/s (each fragment moves this fast relative to center)
    pub fn calculate_separation_velocity(yield_kt: f64, total_mass: f64) -> f64 {
        // Energy of nuclear explosion in Joules
        // 1 kt TNT = 4.184 × 10^12 J
        let energy_j = yield_kt * 4.184e12;

        // Assume 2% of energy goes into kinetic energy of fragments
        // (boosted from realistic ~1% for more dramatic visual separation)
        let kinetic_energy = energy_j * 0.02;

        // KE = 0.5 * m * v^2  →  v = sqrt(2 * KE / m)
        // Using reduced mass for two-body separation
        (2.0 * kinetic_energy / total_mass).sqrt()
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
            DeflectionPayload::NuclearSplit {
                yield_kt,
                split_ratio,
            } => {
                let ratio_percent = (split_ratio * 100.0) as i32;
                if *yield_kt >= 1000.0 {
                    format!(
                        "Nuclear Split ({:.1} Mt, {}/{})",
                        yield_kt / 1000.0,
                        ratio_percent,
                        100 - ratio_percent
                    )
                } else {
                    format!(
                        "Nuclear Split ({:.0} kt, {}/{})",
                        yield_kt,
                        ratio_percent,
                        100 - ratio_percent
                    )
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
        //
        // Note: The dart() function now returns inflated values for gameplay.
        // This test uses explicit real DART parameters.

        let payload = DeflectionPayload::Kinetic {
            mass_kg: 560.0,
            beta: 3.6,
        };
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
        // Reference: 100 kt → 30 cm/s for 300m asteroid (3×10^10 kg)
        // (Boosted ~15× from real physics for gameplay)
        let payload = DeflectionPayload::nuclear(100.0);
        let asteroid_mass = 3e10;
        let direction = DVec2::X;

        let delta_v = payload.calculate_delta_v(asteroid_mass, 0.0, direction);

        assert!(
            (delta_v.length() - 0.30).abs() < 1e-10,
            "100 kt should give 30 cm/s for 3e10 kg asteroid, got {} m/s",
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
        // Test with explicit parameters (dart() now uses inflated values)
        let kinetic = DeflectionPayload::Kinetic {
            mass_kg: 560.0,
            beta: 3.6,
        };
        assert!(kinetic.description().contains("Kinetic"));
        assert!(kinetic.description().contains("560"));

        let nuclear = DeflectionPayload::nuclear(500.0);
        assert!(nuclear.description().contains("Nuclear"));
        assert!(nuclear.description().contains("500 kt"));

        let big_nuclear = DeflectionPayload::nuclear(2000.0);
        assert!(big_nuclear.description().contains("Mt"));
    }

    #[test]
    fn test_nuclear_default() {
        let payload = DeflectionPayload::nuclear_default();
        match payload {
            DeflectionPayload::Nuclear { yield_kt } => {
                assert!(
                    (yield_kt - 12_000.0).abs() < f64::EPSILON,
                    "nuclear_default should have 12000 kt (12 MT) yield, got {yield_kt}"
                );
            }
            _ => panic!("nuclear_default should return Nuclear variant"),
        }
    }

    #[test]
    fn test_nuclear_split_default() {
        let payload = DeflectionPayload::nuclear_split_default();
        match payload {
            DeflectionPayload::NuclearSplit {
                yield_kt,
                split_ratio,
            } => {
                assert!(
                    (yield_kt - 50_000.0).abs() < f64::EPSILON,
                    "nuclear_split_default should have 50000 kt (50 MT) yield, got {yield_kt}"
                );
                assert!(
                    (split_ratio - 0.5).abs() < f64::EPSILON,
                    "nuclear_split_default should have 0.5 split_ratio, got {split_ratio}"
                );
            }
            _ => panic!("nuclear_split_default should return NuclearSplit variant"),
        }
    }

    #[test]
    fn test_nuclear_split_separation_dramatic() {
        // With 2% energy efficiency and 50 MT yield on a 3e10 kg asteroid
        // separation should be visually dramatic (hundreds of m/s)
        let sep_vel = DeflectionPayload::calculate_separation_velocity(50_000.0, 3e10);
        assert!(
            sep_vel > 300.0,
            "Separation should be dramatic (>300 m/s), got {} m/s",
            sep_vel
        );
        assert!(
            sep_vel < 700.0,
            "Separation should be bounded (<700 m/s), got {} m/s",
            sep_vel
        );
    }

    /// Verify that default deflection parameters are effective for gameplay scenarios.
    ///
    /// These tests ensure that 3-5 launches can deflect a typical asteroid when
    /// intercepted at reasonable distances (0.1-0.25 AU from Earth).
    #[test]
    fn test_deflection_gameplay_effectiveness() {
        const AU_TO_METERS: f64 = 1.495978707e11;
        const EARTH_RADIUS_M: f64 = 6.371e6;
        const ASTEROID_VELOCITY: f64 = 29_000.0; // m/s

        // Medium asteroid: 300m, ~3e10 kg
        let asteroid_mass = 3e10;
        let relative_velocity = 30_000.0; // m/s (typical high-speed intercept)
        let direction = DVec2::X;

        // Required delta-v to miss Earth by 2.5× Earth radius at given distance
        let miss_threshold = EARTH_RADIUS_M * 2.5; // ~16,000 km

        // Kinetic impactor: dart() should give ~8 m/s per impact
        let dart = DeflectionPayload::dart();
        let dv_dart = dart
            .calculate_delta_v(asteroid_mass, relative_velocity, direction)
            .length();

        // Nuclear: nuclear_default() should give ~36 m/s per detonation
        let nuclear = DeflectionPayload::nuclear_default();
        let dv_nuclear = nuclear
            .calculate_delta_v(asteroid_mass, relative_velocity, direction)
            .length();

        // Scenario 1: 0.25 AU intercept (15 days out) - 3 kinetic should work
        let required_at_0_25_au = miss_threshold * ASTEROID_VELOCITY / (0.25 * AU_TO_METERS);
        let three_kinetic = dv_dart * 3.0;
        assert!(
            three_kinetic > required_at_0_25_au,
            "3 kinetic impacts ({:.1} m/s) should exceed {:.1} m/s for 0.25 AU",
            three_kinetic,
            required_at_0_25_au
        );

        // Scenario 2: 0.1 AU intercept (6 days out) - single nuclear should work
        let required_at_0_1_au = miss_threshold * ASTEROID_VELOCITY / (0.1 * AU_TO_METERS);
        assert!(
            dv_nuclear > required_at_0_1_au,
            "Single nuclear ({:.1} m/s) should exceed {:.1} m/s for 0.1 AU",
            dv_nuclear,
            required_at_0_1_au
        );

        // Scenario 3: 0.05 AU intercept (3 days out - emergency) - 5 launches needed
        // This is a last-ditch scenario requiring maximum effort: 3 kinetic + 2 nuclear
        let required_at_0_05_au = miss_threshold * ASTEROID_VELOCITY / (0.05 * AU_TO_METERS);
        let emergency_total = dv_dart * 3.0 + dv_nuclear * 2.0;
        assert!(
            emergency_total > required_at_0_05_au,
            "Emergency 3 kinetic + 2 nuclear ({:.1} m/s) should handle 0.05 AU ({:.1} m/s required)",
            emergency_total,
            required_at_0_05_au
        );
    }
}
