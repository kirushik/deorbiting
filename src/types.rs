//! Core physics types and constants for orbital mechanics simulation.

use bevy::prelude::*;
use bevy::math::DVec2;

/// System set for ordering input-related systems.
///
/// Velocity drag must run before position drag to prevent conflicts
/// when the user clicks near the velocity arrow tip.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum InputSystemSet {
    /// Velocity handle drag processing (runs first)
    VelocityDrag,
    /// Position drag processing (runs after velocity drag)
    PositionDrag,
}

/// Physical constants (SI units)

/// Gravitational constant (m³·kg⁻¹·s⁻²)
pub const G: f64 = 6.67430e-11;

/// Astronomical unit in meters
pub const AU_TO_METERS: f64 = 1.495978707e11;

/// Meters to AU
pub const METERS_TO_AU: f64 = 1.0 / AU_TO_METERS;

/// Degrees to radians conversion factor
pub const DEG_TO_RAD: f64 = std::f64::consts::PI / 180.0;

/// Radians to degrees conversion factor
pub const RAD_TO_DEG: f64 = 180.0 / std::f64::consts::PI;

/// Seconds per day
pub const SECONDS_PER_DAY: f64 = 86400.0;

/// J2000.0 epoch as Unix timestamp (January 1, 2000, 12:00 TT)
/// Note: This is approximate; TT differs from UTC by leap seconds
pub const J2000_UNIX: i64 = 946728000;

/// Physical state of a body in the simulation.
/// Uses f64 (DVec2) for physics accuracy over solar system scales.
#[derive(Component, Clone, Debug, Default)]
pub struct BodyState {
    /// Position in meters from solar system barycenter (J2000 ecliptic frame)
    pub pos: DVec2,
    /// Velocity in meters per second
    pub vel: DVec2,
    /// Mass in kilograms
    pub mass: f64,
}

impl BodyState {
    /// Create a new body state
    pub fn new(pos: DVec2, vel: DVec2, mass: f64) -> Self {
        Self { pos, vel, mass }
    }

    /// Position in AU
    pub fn pos_au(&self) -> DVec2 {
        self.pos * METERS_TO_AU
    }

    /// Velocity in AU/day
    pub fn vel_au_per_day(&self) -> DVec2 {
        self.vel * METERS_TO_AU * SECONDS_PER_DAY
    }

    /// Velocity in km/s
    pub fn vel_km_per_s(&self) -> DVec2 {
        self.vel * 0.001
    }
}

/// Represents a selectable body in the simulation (either celestial or asteroid).
///
/// Used by the selection and hover systems to track which body is selected
/// or hovered, regardless of whether it's a celestial body or an asteroid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectableBody {
    /// A celestial body from the ephemeris (Sun, planets, moons)
    Celestial(Entity),
    /// A simulated asteroid
    Asteroid(Entity),
}

impl SelectableBody {
    /// Get the underlying entity regardless of type.
    pub fn entity(&self) -> Entity {
        match self {
            SelectableBody::Celestial(e) | SelectableBody::Asteroid(e) => *e,
        }
    }
}

/// Simulation time resource tracking the current simulation state.
#[derive(Resource, Clone, Debug)]
pub struct SimulationTime {
    /// Current time in seconds since J2000 epoch
    pub current: f64,
    /// Time scale multiplier (1.0 = 1 sim-day per real-second at base rate)
    pub scale: f64,
    /// Whether simulation is paused
    pub paused: bool,
    /// Initial time for reset functionality
    pub initial: f64,
}

impl Default for SimulationTime {
    fn default() -> Self {
        let now = current_j2000_seconds();
        Self {
            current: now,
            scale: 1.0,
            paused: false,
            initial: now,
        }
    }
}

impl SimulationTime {
    /// Create simulation time starting at a specific J2000 seconds value
    pub fn at_j2000_seconds(seconds: f64) -> Self {
        Self {
            current: seconds,
            scale: 1.0,
            paused: false,
            initial: seconds,
        }
    }

    /// Reset to initial time
    pub fn reset(&mut self) {
        self.current = self.initial;
        self.paused = true;
    }

    /// Current time in days since J2000
    pub fn days(&self) -> f64 {
        self.current / SECONDS_PER_DAY
    }
}

/// Convert Unix timestamp to seconds since J2000 epoch
pub fn unix_to_j2000_seconds(unix_timestamp: i64) -> f64 {
    (unix_timestamp - J2000_UNIX) as f64
}

/// Convert J2000 seconds to Unix timestamp
pub fn j2000_seconds_to_unix(j2000_seconds: f64) -> i64 {
    J2000_UNIX + j2000_seconds as i64
}

/// Get current time as J2000 seconds (using system clock)
pub fn current_j2000_seconds() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let unix_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    unix_to_j2000_seconds(unix_now)
}

/// Format J2000 seconds as a human-readable date string.
/// Returns format: "YYYY-MM-DD HH:MM:SS UTC (approx)"
///
/// **Note:** This is an approximation for display purposes only.
/// - Does not account for leap seconds (~27 seconds cumulative since 1972)
/// - Does not convert from TT (Terrestrial Time) to UTC (~69 seconds offset)
/// - J2000 epoch is defined in TT, not UTC
///
/// For mission-critical time display, use a proper astronomical time library.
pub fn j2000_seconds_to_date_string(j2000_seconds: f64) -> String {
    let unix_secs = j2000_seconds_to_unix(j2000_seconds);

    // Simple date calculation - approximate, ignores leap seconds and TT/UTC offset
    let days_since_epoch = unix_secs / 86400;
    let time_of_day = unix_secs % 86400;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year, month, day from days since Unix epoch (Jan 1, 1970)
    let (year, month, day) = days_to_ymd(days_since_epoch);

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC (approx)",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to year, month, day
fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // Algorithm for Gregorian calendar
    let mut remaining_days = days + 719468; // Days from year 0 to 1970

    let era = if remaining_days >= 0 {
        remaining_days / 146097
    } else {
        (remaining_days - 146096) / 146097
    };

    let day_of_era = (remaining_days - era * 146097) as u32;
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let year = (year_of_era as i64 + era * 400) as i32;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unix_to_j2000() {
        // J2000 epoch should give 0
        assert_eq!(unix_to_j2000_seconds(J2000_UNIX), 0.0);

        // One day after J2000
        let one_day_later = J2000_UNIX + 86400;
        assert_eq!(unix_to_j2000_seconds(one_day_later), 86400.0);
    }

    #[test]
    fn test_j2000_to_unix() {
        assert_eq!(j2000_seconds_to_unix(0.0), J2000_UNIX);
        assert_eq!(j2000_seconds_to_unix(86400.0), J2000_UNIX + 86400);
    }

    #[test]
    fn test_date_string_j2000() {
        // J2000 epoch should be January 1, 2000, 12:00:00 UTC
        let date_str = j2000_seconds_to_date_string(0.0);
        assert!(date_str.contains("2000-01-01"), "Expected 2000-01-01, got {}", date_str);
        assert!(date_str.contains("12:00:00"), "Expected 12:00:00, got {}", date_str);
    }

    #[test]
    fn test_unit_conversions() {
        // 1 AU should convert to correct meters
        let one_au_meters = 1.0 * AU_TO_METERS;
        assert!((one_au_meters - 1.495978707e11).abs() < 1.0);

        // Round trip
        let au = one_au_meters * METERS_TO_AU;
        assert!((au - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_body_state_conversions() {
        let state = BodyState::new(
            DVec2::new(AU_TO_METERS, 0.0), // 1 AU on x-axis
            DVec2::new(0.0, 29780.0),      // ~Earth orbital velocity
            5.972e24,                       // Earth mass
        );

        let pos_au = state.pos_au();
        assert!((pos_au.x - 1.0).abs() < 1e-10);
        assert!(pos_au.y.abs() < 1e-10);

        let vel_km_s = state.vel_km_per_s();
        assert!((vel_km_s.y - 29.78).abs() < 0.01);
    }

    #[test]
    fn test_simulation_time_default() {
        let sim_time = SimulationTime::default();
        assert!(!sim_time.paused);
        assert_eq!(sim_time.scale, 1.0);
        // Current time should be reasonably close to now
        assert!(sim_time.current > 0.0); // We're past J2000
    }
}
