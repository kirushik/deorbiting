//! Preset scenario definitions.
//!
//! Six educational scenarios covering various orbital mechanics concepts.
//! All scenarios use dynamic computation based on current planet positions.

use crate::ephemeris::CelestialBodyId;

use super::{CameraTarget, Scenario};

/// All available preset scenarios.
pub static SCENARIOS: &[Scenario] = &[
    EARTH_COLLISION,
    APOPHIS_FLYBY,
    JUPITER_SLINGSHOT,
    INTERSTELLAR_VISITOR,
    DEFLECTION_CHALLENGE,
    SANDBOX,
];

/// Scenario 1: Earth Collision Course (Default/Tutorial)
///
/// An asteroid is positioned ahead of Earth in its orbit, moving retrograde.
/// This creates a collision course with Earth in approximately 23 days.
/// Perfect for understanding collision mechanics and the basics of interception.
pub static EARTH_COLLISION: Scenario = Scenario {
    id: "earth_collision",
    name: "Earth Collision Course",
    description: "Asteroid on collision course with Earth (~23 days). Tutorial scenario.",
    asteroid_pos: None,   // Computed dynamically: 45° ahead of Earth
    asteroid_vel: None,   // Computed dynamically: retrograde at Earth's orbital velocity
    asteroid_mass: 1e12,  // ~1 billion tons
    asteroid_radius: 2.0, // Visual size
    start_time: None,
    time_scale: 10.0,
    start_paused: true, // Start paused so user can look around
    camera_target: CameraTarget::Body(CelestialBodyId::Earth),
    camera_zoom: 0.5,
};

/// Scenario 2: Apophis Flyby (Gravity Assist Demo)
///
/// Simulates a close Earth approach similar to Apophis (2029 flyby).
/// The asteroid passes close to Earth, demonstrating gravitational deflection.
/// Shows how close encounters change orbital parameters.
pub static APOPHIS_FLYBY: Scenario = Scenario {
    id: "apophis_flyby",
    name: "Apophis Flyby",
    description: "Close Earth approach. Watch gravity bend the trajectory.",
    asteroid_pos: None,    // Computed dynamically: ahead of Earth, outside orbit
    asteroid_vel: None,    // Computed dynamically: retrograde with inward component
    asteroid_mass: 2.7e10, // Apophis mass (~27 million tons)
    asteroid_radius: 2.0,
    start_time: None,
    time_scale: 10.0,
    start_paused: true,
    camera_target: CameraTarget::Body(CelestialBodyId::Earth),
    camera_zoom: 0.3, // Close view for the flyby
};

/// Scenario 3: Jupiter Slingshot (Classic Gravity Assist)
///
/// Asteroid approaches Jupiter from behind, gaining velocity.
/// Classic Voyager-style gravity assist maneuver.
/// Demonstrates energy transfer from planet to spacecraft.
pub static JUPITER_SLINGSHOT: Scenario = Scenario {
    id: "jupiter_slingshot",
    name: "Jupiter Slingshot",
    description: "Gravity assist at Jupiter. Watch energy transfer from the planet.",
    asteroid_pos: None, // Computed dynamically: behind Jupiter, inside its orbit
    asteroid_vel: None, // Computed dynamically: prograde, faster than circular
    asteroid_mass: 5e11,
    asteroid_radius: 2.0,
    start_time: None,
    time_scale: 100.0, // Faster for Jupiter's longer orbital period
    start_paused: true,
    camera_target: CameraTarget::Body(CelestialBodyId::Jupiter),
    camera_zoom: 1.5, // Wider view for outer solar system
};

/// Scenario 4: Interstellar Visitor (Oumuamua-style)
///
/// A hyperbolic trajectory (e > 1) at ~40 km/s.
/// Demonstrates escape trajectories and interstellar origins.
/// The asteroid will leave the solar system permanently.
pub static INTERSTELLAR_VISITOR: Scenario = Scenario {
    id: "interstellar_visitor",
    name: "Interstellar Visitor",
    description: "Hyperbolic escape trajectory at 40 km/s. 'Oumuamua-style visitor.",
    asteroid_pos: None, // Computed dynamically: approaching from outer solar system
    asteroid_vel: None, // Computed dynamically: ~40 km/s toward inner system
    asteroid_mass: 4e9, // Small, like 'Oumuamua
    asteroid_radius: 2.0,
    start_time: None,
    time_scale: 50.0,
    start_paused: true,
    camera_target: CameraTarget::Sun,
    camera_zoom: 2.0, // Wide view to see the whole trajectory
};

/// Scenario 5: Deflection Challenge (Planetary Defense Game)
///
/// Asteroid on collision course with ~46 day lead time.
/// Goal: Apply minimal delta-v to make asteroid miss Earth.
/// Educational: tiny changes early = huge miss distance.
pub static DEFLECTION_CHALLENGE: Scenario = Scenario {
    id: "deflection_challenge",
    name: "Deflection Challenge",
    description: "~46 day warning. Can you deflect it with minimal delta-v?",
    asteroid_pos: None, // Computed dynamically: 90° ahead of Earth
    asteroid_vel: None, // Computed dynamically: retrograde at Earth's orbital velocity
    asteroid_mass: 5e9, // ~150m diameter rock (solvable with nuclear/heavy kinetic)
    asteroid_radius: 2.0,
    start_time: None,
    time_scale: 10.0,
    start_paused: true, // Start paused so player can plan
    camera_target: CameraTarget::Sun,
    camera_zoom: 0.6, // View both Earth and asteroid
};

/// Scenario 6: Sandbox (Free Experimentation)
///
/// Asteroid near Earth with zero initial velocity.
/// User can experiment with the velocity handle.
/// Perfect for understanding orbital mechanics intuitively.
pub static SANDBOX: Scenario = Scenario {
    id: "sandbox",
    name: "Sandbox",
    description: "Free experimentation. Drag the velocity to create any orbit.",
    asteroid_pos: None, // Computed dynamically: near Earth
    asteroid_vel: None, // Computed dynamically: zero velocity
    asteroid_mass: 1e12,
    asteroid_radius: 2.0,
    start_time: None,
    time_scale: 10.0,
    start_paused: true, // Start paused for experimentation
    camera_target: CameraTarget::Asteroid,
    camera_zoom: 0.5,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenarios_have_unique_ids() {
        let mut ids: Vec<&str> = SCENARIOS.iter().map(|s| s.id).collect();
        let original_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), original_len, "Scenario IDs must be unique");
    }

    #[test]
    fn test_scenario_count() {
        assert_eq!(SCENARIOS.len(), 6, "Should have exactly 6 scenarios");
    }

    #[test]
    fn test_earth_collision_is_default() {
        assert_eq!(SCENARIOS[0].id, "earth_collision");
    }

    #[test]
    fn test_all_scenarios_start_paused() {
        for scenario in SCENARIOS.iter() {
            assert!(
                scenario.start_paused,
                "Scenario {} should start paused",
                scenario.id
            );
        }
    }

    #[test]
    fn test_all_scenarios_use_dynamic_computation() {
        for scenario in SCENARIOS.iter() {
            assert!(
                scenario.asteroid_pos.is_none() && scenario.asteroid_vel.is_none(),
                "Scenario {} should use dynamic computation (None for pos/vel)",
                scenario.id
            );
        }
    }
}
