//! Shared deflection method definitions and utilities.
//!
//! This module consolidates deflection-related code that was previously
//! duplicated across banners.rs, context_card.rs, and radial_menu.rs.

use bevy::math::DVec2;
use bevy::prelude::*;
use bevy_egui::egui;

use crate::continuous::{ContinuousPayload, LaunchContinuousDeflectorEvent};
use crate::interceptor::{DeflectionPayload, LaunchInterceptorEvent};
use crate::types::BodyState;

use super::icons;

/// Base interceptor speed in m/s.
///
/// Used to estimate flight time for deflection missions.
/// Inflated for gameplay balance (~7× realistic) to give reasonable response times.
/// At 100 km/s, an interceptor reaches:
/// - 0.5 AU (~Earth-Mars) in ~9 days
/// - 1.0 AU in ~17 days
/// - 2.0 AU in ~35 days
pub const BASE_INTERCEPTOR_SPEED: f64 = 100_000.0;

/// Available deflection methods.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeflectionMethod {
    // Instant methods
    Kinetic,
    Nuclear,
    NuclearSplit,
    // Continuous methods
    IonBeam,
    LaserAblation,
    SolarSail,
}

impl DeflectionMethod {
    /// Get the icon for this deflection method.
    pub fn icon(&self) -> &'static str {
        match self {
            DeflectionMethod::Kinetic => icons::KINETIC,
            DeflectionMethod::Nuclear => icons::NUCLEAR,
            DeflectionMethod::NuclearSplit => icons::NUCLEAR_SPLIT,
            DeflectionMethod::IonBeam => icons::ION_BEAM,
            DeflectionMethod::LaserAblation => icons::LASER,
            DeflectionMethod::SolarSail => icons::SOLAR_SAIL,
        }
    }

    /// Get the display name for this deflection method.
    pub fn name(&self) -> &'static str {
        match self {
            DeflectionMethod::Kinetic => "Kinetic",
            DeflectionMethod::Nuclear => "Nuclear",
            DeflectionMethod::NuclearSplit => "Split",
            DeflectionMethod::IonBeam => "Ion Beam",
            DeflectionMethod::LaserAblation => "Laser",
            DeflectionMethod::SolarSail => "Solar Sail",
        }
    }

    /// Get the accent color for this deflection method.
    pub fn color(&self) -> egui::Color32 {
        match self {
            DeflectionMethod::Kinetic => egui::Color32::from_rgb(255, 180, 100),
            DeflectionMethod::Nuclear => egui::Color32::from_rgb(255, 100, 100),
            DeflectionMethod::NuclearSplit => egui::Color32::from_rgb(255, 80, 150),
            DeflectionMethod::IonBeam => egui::Color32::from_rgb(100, 200, 255),
            DeflectionMethod::LaserAblation => egui::Color32::from_rgb(255, 200, 80),
            DeflectionMethod::SolarSail => egui::Color32::from_rgb(255, 230, 100),
        }
    }

    /// Check if this is a continuous deflection method.
    pub fn is_continuous(&self) -> bool {
        matches!(
            self,
            DeflectionMethod::IonBeam
                | DeflectionMethod::LaserAblation
                | DeflectionMethod::SolarSail
        )
    }
}

/// All deflection methods available.
pub const ALL_METHODS: [DeflectionMethod; 6] = [
    DeflectionMethod::Kinetic,
    DeflectionMethod::Nuclear,
    DeflectionMethod::NuclearSplit,
    DeflectionMethod::IonBeam,
    DeflectionMethod::LaserAblation,
    DeflectionMethod::SolarSail,
];

/// Calculate flight time from Earth to asteroid position.
///
/// # Arguments
/// * `asteroid_pos` - Asteroid position in meters
/// * `earth_pos` - Earth position in meters
///
/// # Returns
/// Flight time in seconds
pub fn calculate_flight_time_from_earth(asteroid_pos: DVec2, earth_pos: DVec2) -> f64 {
    (asteroid_pos - earth_pos).length() / BASE_INTERCEPTOR_SPEED
}

/// Apply a deflection method with default parameters.
///
/// Parameters are inflated for effectiveness at gameplay timescales.
/// Uses factory methods to ensure consistency with payload defaults.
pub fn apply_deflection(
    target: Entity,
    method: DeflectionMethod,
    asteroid_state: &BodyState,
    flight_time_seconds: f64,
    launch_events: &mut MessageWriter<LaunchInterceptorEvent>,
    continuous_launch_events: &mut MessageWriter<LaunchContinuousDeflectorEvent>,
) {
    // Default direction: retrograde (opposite to velocity)
    let direction = -asteroid_state.vel.normalize_or_zero();

    match method {
        DeflectionMethod::Kinetic => {
            launch_events.write(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::dart(),
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::Nuclear => {
            launch_events.write(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::nuclear_default(),
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::NuclearSplit => {
            launch_events.write(LaunchInterceptorEvent {
                target,
                payload: DeflectionPayload::nuclear_split_default(),
                direction: Some(direction),
                flight_time: Some(flight_time_seconds),
            });
        }
        DeflectionMethod::IonBeam => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::ion_beam_default(),
                flight_time: flight_time_seconds,
            });
        }
        DeflectionMethod::LaserAblation => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::laser_ablation_default(),
                flight_time: 0.0, // Instant - Earth-based laser platform (DE-STAR concept)
            });
        }
        DeflectionMethod::SolarSail => {
            continuous_launch_events.write(LaunchContinuousDeflectorEvent {
                target,
                payload: ContinuousPayload::solar_sail_default(),
                flight_time: flight_time_seconds,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flight_time_calculation() {
        let earth_pos = DVec2::new(0.0, 0.0);
        let asteroid_pos = DVec2::new(BASE_INTERCEPTOR_SPEED * 100.0, 0.0); // 100 seconds away

        let flight_time = calculate_flight_time_from_earth(asteroid_pos, earth_pos);

        assert!(
            (flight_time - 100.0).abs() < 1e-10,
            "Expected 100 seconds, got {flight_time}"
        );
    }

    #[test]
    fn test_flight_time_diagonal() {
        let earth_pos = DVec2::ZERO;
        // Create a point at 45 degrees, 1 km away
        let distance = 1000.0;
        let asteroid_pos = DVec2::new(distance / 2.0_f64.sqrt(), distance / 2.0_f64.sqrt());

        let flight_time = calculate_flight_time_from_earth(asteroid_pos, earth_pos);
        let expected = distance / BASE_INTERCEPTOR_SPEED;

        assert!(
            (flight_time - expected).abs() < 1e-10,
            "Expected {expected} seconds, got {flight_time}"
        );
    }

    #[test]
    fn test_all_methods_count() {
        assert_eq!(
            ALL_METHODS.len(),
            6,
            "Should have exactly 6 deflection methods"
        );
    }

    #[test]
    fn test_continuous_methods() {
        // Instant methods
        assert!(!DeflectionMethod::Kinetic.is_continuous());
        assert!(!DeflectionMethod::Nuclear.is_continuous());
        assert!(!DeflectionMethod::NuclearSplit.is_continuous());

        // Continuous methods
        assert!(DeflectionMethod::IonBeam.is_continuous());
        assert!(DeflectionMethod::LaserAblation.is_continuous());
        assert!(DeflectionMethod::SolarSail.is_continuous());
    }

    #[test]
    fn test_method_names() {
        assert_eq!(DeflectionMethod::Kinetic.name(), "Kinetic");
        assert_eq!(DeflectionMethod::Nuclear.name(), "Nuclear");
        assert_eq!(DeflectionMethod::NuclearSplit.name(), "Split");
        assert_eq!(DeflectionMethod::IonBeam.name(), "Ion Beam");
        assert_eq!(DeflectionMethod::LaserAblation.name(), "Laser");
        assert_eq!(DeflectionMethod::SolarSail.name(), "Solar Sail");
    }

    /// Verify flight times are reasonable for gameplay scenarios.
    /// These tests ensure interceptors reach targets in days/weeks, not months.
    #[test]
    fn test_flight_time_gameplay_scenarios() {
        const AU_TO_METERS: f64 = 1.495978707e11;
        const SECONDS_PER_DAY: f64 = 86400.0;

        // Earth at 1 AU
        let earth_pos = DVec2::new(AU_TO_METERS, 0.0);

        // Scenario 1: Near-Earth asteroid (0.1 AU away)
        // Should be reachable in ~2 days
        let near_earth = DVec2::new(AU_TO_METERS + 0.1 * AU_TO_METERS, 0.0);
        let flight_days = calculate_flight_time_from_earth(near_earth, earth_pos) / SECONDS_PER_DAY;
        assert!(
            flight_days < 5.0,
            "Near-Earth (0.1 AU) should be < 5 days, got {:.1} days",
            flight_days
        );

        // Scenario 2: Half AU away (typical deflection challenge)
        // Should be reachable in ~10 days
        let mid_range = DVec2::new(AU_TO_METERS + 0.5 * AU_TO_METERS, 0.0);
        let flight_days = calculate_flight_time_from_earth(mid_range, earth_pos) / SECONDS_PER_DAY;
        assert!(
            flight_days < 15.0,
            "Mid-range (0.5 AU) should be < 15 days, got {:.1} days",
            flight_days
        );

        // Scenario 3: Opposite side of orbit (2 AU away)
        // Should be reachable in ~35 days
        let far_side = DVec2::new(-AU_TO_METERS, 0.0);
        let flight_days = calculate_flight_time_from_earth(far_side, earth_pos) / SECONDS_PER_DAY;
        assert!(
            flight_days < 50.0,
            "Far side (2 AU) should be < 50 days, got {:.1} days",
            flight_days
        );
    }
}
