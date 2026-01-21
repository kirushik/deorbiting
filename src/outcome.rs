//! Trajectory outcome detection for orbital mechanics.
//!
//! Provides orbital mechanics calculations to classify trajectory outcomes:
//! - Collision: asteroid hits a celestial body
//! - Escape: hyperbolic trajectory leaving the solar system
//! - Stable orbit: bound orbit without collision

use bevy::math::DVec2;

use crate::ephemeris::CelestialBodyId;
use crate::types::GM_SUN;

/// Outcome of a predicted trajectory.
#[derive(Clone, Debug, Default)]
pub enum TrajectoryOutcome {
    /// Prediction in progress, outcome not yet determined.
    #[default]
    InProgress,

    /// Asteroid will collide with a celestial body.
    Collision {
        /// Body that will be hit.
        body_hit: CelestialBodyId,
        /// Time until impact (seconds).
        time_to_impact: f64,
        /// Impact velocity magnitude (m/s).
        impact_velocity: f64,
    },

    /// Asteroid on escape trajectory (hyperbolic, E > 0).
    Escape {
        /// Excess velocity at infinity (m/s).
        v_infinity: f64,
        /// Direction of escape (unit vector).
        direction: DVec2,
    },

    /// Asteroid in stable bound orbit (E < 0).
    StableOrbit {
        /// Semi-major axis (meters).
        semi_major_axis: f64,
        /// Eccentricity (0 = circular, <1 = elliptical).
        eccentricity: f64,
        /// Orbital period (seconds).
        period: f64,
        /// Perihelion distance (meters).
        perihelion: f64,
        /// Aphelion distance (meters).
        aphelion: f64,
    },
}

/// Orbital elements computed from state vectors.
#[derive(Clone, Debug)]
pub struct OrbitalElements {
    /// Semi-major axis (meters). Negative for hyperbolic orbits.
    pub semi_major_axis: f64,
    /// Eccentricity (0 = circular, <1 = elliptical, =1 = parabolic, >1 = hyperbolic).
    pub eccentricity: f64,
    /// Specific orbital energy (m²/s²). E > 0 = hyperbolic.
    pub energy: f64,
    /// Specific angular momentum magnitude (m²/s).
    pub angular_momentum: f64,
    /// Orbital period (seconds). Only valid for bound orbits (E < 0).
    pub period: Option<f64>,
}

impl OrbitalElements {
    /// Returns true if orbit is bound (E < 0).
    pub fn is_bound(&self) -> bool {
        self.energy < 0.0
    }

    /// Returns true if orbit is hyperbolic (E > 0).
    pub fn is_hyperbolic(&self) -> bool {
        self.energy > 0.0
    }

    /// Perihelion distance (closest approach to Sun).
    pub fn perihelion(&self) -> f64 {
        self.semi_major_axis * (1.0 - self.eccentricity)
    }

    /// Aphelion distance (farthest from Sun). Only meaningful for bound orbits.
    pub fn aphelion(&self) -> f64 {
        self.semi_major_axis * (1.0 + self.eccentricity)
    }

    /// Excess velocity at infinity for hyperbolic orbits (m/s).
    /// Returns 0 for bound orbits.
    pub fn v_infinity(&self) -> f64 {
        if self.energy > 0.0 {
            (2.0 * self.energy).sqrt()
        } else {
            0.0
        }
    }
}

/// Calculate specific orbital energy.
///
/// E = v²/2 - GM/r
/// - E < 0: bound orbit (elliptical)
/// - E = 0: parabolic (escape at infinity with zero velocity)
/// - E > 0: hyperbolic (escape with excess velocity)
///
/// # Arguments
/// * `pos` - Position vector from Sun (meters)
/// * `vel` - Velocity vector (m/s)
/// * `gm` - Standard gravitational parameter (m³/s²), defaults to GM_SUN
///
/// # Returns
/// `None` if position is at or very near the origin (inside Sun)
pub fn orbital_energy(pos: DVec2, vel: DVec2, gm: f64) -> Option<f64> {
    let r = pos.length();
    // Guard against division by zero (position at Sun center)
    if r < 1e6 {
        return None; // Less than 1km from Sun center
    }
    let v_sq = vel.length_squared();
    Some(0.5 * v_sq - gm / r)
}

/// Calculate specific angular momentum magnitude.
///
/// h = |r × v| (in 2D, this is r.x*v.y - r.y*v.x)
pub fn angular_momentum(pos: DVec2, vel: DVec2) -> f64 {
    (pos.x * vel.y - pos.y * vel.x).abs()
}

/// Compute orbital elements from position and velocity state vectors.
///
/// Uses vis-viva equations and angular momentum to derive all orbital parameters.
///
/// # Arguments
/// * `pos` - Position vector from Sun (meters)
/// * `vel` - Velocity vector (m/s)
/// * `gm` - Standard gravitational parameter (m³/s²), defaults to GM_SUN
///
/// # Returns
/// `None` if position is at or very near the origin (inside Sun)
pub fn compute_orbital_elements(pos: DVec2, vel: DVec2, gm: f64) -> Option<OrbitalElements> {
    let r = pos.length();
    // Guard against division by zero
    if r < 1e6 {
        return None; // Less than 1km from Sun center
    }
    
    let v = vel.length();
    let v_sq = v * v;

    // Specific orbital energy
    let energy = 0.5 * v_sq - gm / r;

    // Specific angular momentum
    let h = angular_momentum(pos, vel);

    // Semi-major axis from vis-viva: a = -GM / (2E)
    // For hyperbolic orbits, a is negative
    let semi_major_axis = if energy.abs() > 1e-10 {
        -gm / (2.0 * energy)
    } else {
        // Near-parabolic: a approaches infinity
        f64::INFINITY
    };

    // Eccentricity from e = sqrt(1 + 2Eh²/GM²)
    let e_squared = 1.0 + (2.0 * energy * h * h) / (gm * gm);
    let eccentricity = if e_squared > 0.0 {
        e_squared.sqrt()
    } else {
        0.0 // Numerical safety for near-circular
    };

    // Orbital period for bound orbits: T = 2π * sqrt(a³/GM)
    let period = if energy < 0.0 && semi_major_axis > 0.0 {
        let a_cubed = semi_major_axis.powi(3);
        Some(2.0 * std::f64::consts::PI * (a_cubed / gm).sqrt())
    } else {
        None
    };

    Some(OrbitalElements {
        semi_major_axis,
        eccentricity,
        energy,
        angular_momentum: h,
        period,
    })
}

/// Determine trajectory outcome from prediction results.
///
/// Classifies the trajectory into one of three outcomes:
/// - Collision: if prediction ended in collision
/// - Escape: if specific orbital energy E > 0 and moving outward
/// - Stable orbit: if E < 0 and completed prediction without collision
///
/// # Arguments
/// * `initial_pos` - Starting position (meters from Sun)
/// * `initial_vel` - Starting velocity (m/s)
/// * `ends_in_collision` - Whether prediction ended in collision
/// * `collision_target` - Body hit (if collision)
/// * `final_pos` - Final position from prediction
/// * `final_vel` - Final velocity from prediction
/// * `prediction_time_span` - How long the prediction ran (seconds)
/// * `impact_velocity` - Velocity at impact (if collision)
pub fn detect_outcome(
    initial_pos: DVec2,
    initial_vel: DVec2,
    ends_in_collision: bool,
    collision_target: Option<CelestialBodyId>,
    _final_pos: DVec2,
    final_vel: DVec2,
    prediction_time_span: f64,
    impact_velocity: Option<f64>,
) -> TrajectoryOutcome {
    // Case 1: Collision detected
    if ends_in_collision
        && let Some(body) = collision_target {
            return TrajectoryOutcome::Collision {
                body_hit: body,
                time_to_impact: prediction_time_span,
                impact_velocity: impact_velocity.unwrap_or(final_vel.length()),
            };
        }

    // Compute orbital elements from initial state (relative to Sun)
    // Returns None if position is at origin (shouldn't happen in practice)
    let Some(elements) = compute_orbital_elements(initial_pos, initial_vel, GM_SUN) else {
        return TrajectoryOutcome::InProgress;
    };

    // Case 2: Escape trajectory (E > 0)
    if elements.is_hyperbolic() {
        // Direction is the velocity direction (asymptotic direction)
        let direction = initial_vel.normalize_or_zero();
        return TrajectoryOutcome::Escape {
            v_infinity: elements.v_infinity(),
            direction,
        };
    }

    // Case 3: Bound orbit
    // Consider it "stable" if we've simulated a significant portion of the orbit
    // without collision
    if let Some(period) = elements.period {
        // We consider the orbit characterized if we've simulated at least 10%
        // of the period or 30 days, whichever is smaller
        let min_time = (period * 0.1).min(30.0 * 86400.0);
        if prediction_time_span >= min_time {
            return TrajectoryOutcome::StableOrbit {
                semi_major_axis: elements.semi_major_axis,
                eccentricity: elements.eccentricity,
                period,
                perihelion: elements.perihelion(),
                aphelion: elements.aphelion(),
            };
        }
    }

    // Still computing or not enough data
    TrajectoryOutcome::InProgress
}

impl TrajectoryOutcome {
    /// Returns true if the outcome is determined (not InProgress).
    pub fn is_determined(&self) -> bool {
        !matches!(self, TrajectoryOutcome::InProgress)
    }

    /// Returns true if this is a collision outcome.
    pub fn is_collision(&self) -> bool {
        matches!(self, TrajectoryOutcome::Collision { .. })
    }

    /// Returns true if this is an escape outcome.
    pub fn is_escape(&self) -> bool {
        matches!(self, TrajectoryOutcome::Escape { .. })
    }

    /// Returns true if this is a stable orbit outcome.
    pub fn is_stable(&self) -> bool {
        matches!(self, TrajectoryOutcome::StableOrbit { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AU_TO_METERS;

    #[test]
    fn test_orbital_energy_circular() {
        // Circular orbit at 1 AU
        let r = AU_TO_METERS;
        let pos = DVec2::new(r, 0.0);

        // Circular velocity: v = sqrt(GM/r)
        let v_circular = (GM_SUN / r).sqrt();
        let vel = DVec2::new(0.0, v_circular);

        let energy = orbital_energy(pos, vel, GM_SUN).expect("valid position");

        // For circular orbit: E = -GM/(2r)
        let expected = -GM_SUN / (2.0 * r);
        let relative_error = (energy - expected).abs() / expected.abs();
        assert!(
            relative_error < 1e-10,
            "Circular orbit energy error: {relative_error}"
        );
    }

    #[test]
    fn test_orbital_energy_escape() {
        // Position at 1 AU
        let r = AU_TO_METERS;
        let pos = DVec2::new(r, 0.0);

        // Escape velocity: v_esc = sqrt(2GM/r)
        let v_escape = (2.0 * GM_SUN / r).sqrt();

        // Test at exactly escape velocity
        let vel = DVec2::new(0.0, v_escape);
        let energy = orbital_energy(pos, vel, GM_SUN).expect("valid position");
        assert!(
            energy.abs() < 1e10,
            "Escape velocity should give E ≈ 0, got {energy}"
        );

        // Test above escape velocity
        let vel_fast = DVec2::new(0.0, v_escape * 1.5);
        let energy_fast = orbital_energy(pos, vel_fast, GM_SUN).expect("valid position");
        assert!(
            energy_fast > 0.0,
            "Above escape velocity should give E > 0, got {energy_fast}"
        );
    }

    #[test]
    fn test_orbital_elements_earth_like() {
        // Earth-like circular orbit at 1 AU
        let r = AU_TO_METERS;
        let pos = DVec2::new(r, 0.0);
        let v_circular = (GM_SUN / r).sqrt();
        let vel = DVec2::new(0.0, v_circular);

        let elements = compute_orbital_elements(pos, vel, GM_SUN).expect("valid position");

        // Semi-major axis should be ~1 AU
        let a_error = (elements.semi_major_axis - r).abs() / r;
        assert!(
            a_error < 1e-10,
            "Semi-major axis error: {a_error}, got {} AU",
            elements.semi_major_axis / AU_TO_METERS
        );

        // Eccentricity should be ~0
        assert!(
            elements.eccentricity < 1e-6,
            "Eccentricity should be ~0, got {}",
            elements.eccentricity
        );

        // Period should be ~1 year (365.25 days ≈ 3.156e7 seconds)
        let period = elements.period.expect("Bound orbit should have period");
        let year_seconds = 365.25 * 86400.0;
        let period_error = (period - year_seconds).abs() / year_seconds;
        assert!(
            period_error < 0.01,
            "Period error: {period_error}, got {} days",
            period / 86400.0
        );
    }

    #[test]
    fn test_orbital_elements_elliptical() {
        // Elliptical orbit: start at perihelion with higher velocity
        let r_peri = 0.5 * AU_TO_METERS; // Perihelion at 0.5 AU
        let pos = DVec2::new(r_peri, 0.0);

        // For an orbit with a = 1 AU and perihelion at 0.5 AU:
        // e = 1 - r_peri/a = 0.5
        // v_peri = sqrt(GM * (2/r - 1/a))
        let a = AU_TO_METERS;
        let v_peri = (GM_SUN * (2.0 / r_peri - 1.0 / a)).sqrt();
        let vel = DVec2::new(0.0, v_peri);

        let elements = compute_orbital_elements(pos, vel, GM_SUN).expect("valid position");

        // Check semi-major axis
        let a_error = (elements.semi_major_axis - a).abs() / a;
        assert!(
            a_error < 1e-6,
            "Semi-major axis error: {a_error}, got {} AU",
            elements.semi_major_axis / AU_TO_METERS
        );

        // Check eccentricity (should be ~0.5)
        let e_expected = 0.5;
        let e_error = (elements.eccentricity - e_expected).abs();
        assert!(
            e_error < 1e-6,
            "Eccentricity error: {e_error}, got {}",
            elements.eccentricity
        );
    }

    #[test]
    fn test_hyperbolic_elements() {
        // Hyperbolic trajectory: high velocity at 1 AU
        let r = AU_TO_METERS;
        let pos = DVec2::new(r, 0.0);

        // Velocity well above escape (50 km/s)
        let vel = DVec2::new(0.0, 50_000.0);

        let elements = compute_orbital_elements(pos, vel, GM_SUN).expect("valid position");

        assert!(elements.is_hyperbolic(), "Should be hyperbolic trajectory");
        assert!(
            elements.eccentricity > 1.0,
            "Hyperbolic e > 1, got {}",
            elements.eccentricity
        );
        assert!(
            elements.period.is_none(),
            "Hyperbolic orbit has no period"
        );
        assert!(
            elements.v_infinity() > 0.0,
            "Should have excess velocity"
        );
    }


    #[test]
    fn test_orbital_energy_at_origin() {
        // Test that orbital_energy returns None when position is at origin
        let pos = DVec2::ZERO;
        let vel = DVec2::new(1000.0, 0.0);
        
        assert!(
            orbital_energy(pos, vel, GM_SUN).is_none(),
            "Should return None for position at origin"
        );
        
        // Also test compute_orbital_elements
        assert!(
            compute_orbital_elements(pos, vel, GM_SUN).is_none(),
            "Should return None for position at origin"
        );
    }

    #[test]
    fn test_detect_collision_outcome() {
        let pos = DVec2::new(AU_TO_METERS, 0.0);
        let vel = DVec2::new(-30_000.0, 0.0);

        let outcome = detect_outcome(
            pos,
            vel,
            true,
            Some(CelestialBodyId::Earth),
            pos, // final pos doesn't matter for collision
            vel,
            20.0 * 86400.0, // 20 days
            Some(30_000.0),
        );

        match outcome {
            TrajectoryOutcome::Collision {
                body_hit,
                time_to_impact,
                impact_velocity,
            } => {
                assert_eq!(body_hit, CelestialBodyId::Earth);
                assert!((time_to_impact - 20.0 * 86400.0).abs() < 1.0);
                assert!((impact_velocity - 30_000.0).abs() < 1.0);
            }
            _ => panic!("Expected collision outcome, got {outcome:?}"),
        }
    }

    #[test]
    fn test_detect_escape_outcome() {
        let pos = DVec2::new(AU_TO_METERS, 0.0);
        // Very high velocity -> hyperbolic
        let vel = DVec2::new(0.0, 50_000.0);

        let outcome = detect_outcome(
            pos,
            vel,
            false,
            None,
            DVec2::new(10.0 * AU_TO_METERS, 0.0),
            vel,
            365.0 * 86400.0,
            None,
        );

        assert!(outcome.is_escape(), "Expected escape outcome, got {outcome:?}");
    }

    #[test]
    fn test_detect_stable_orbit_outcome() {
        let r = AU_TO_METERS;
        let pos = DVec2::new(r, 0.0);
        let v_circular = (GM_SUN / r).sqrt();
        let vel = DVec2::new(0.0, v_circular);

        // Simulate for 100 days (about 27% of a year)
        let sim_time = 100.0 * 86400.0;

        let outcome = detect_outcome(pos, vel, false, None, pos, vel, sim_time, None);

        match outcome {
            TrajectoryOutcome::StableOrbit {
                semi_major_axis,
                eccentricity,
                period,
                ..
            } => {
                assert!(
                    (semi_major_axis - r).abs() / r < 0.01,
                    "a should be ~1 AU"
                );
                assert!(eccentricity < 0.01, "e should be ~0");
                assert!(
                    (period - 365.25 * 86400.0).abs() / (365.25 * 86400.0) < 0.01,
                    "T should be ~1 year"
                );
            }
            _ => panic!("Expected stable orbit outcome, got {outcome:?}"),
        }
    }
}
