//! Thrust calculation functions for continuous deflection methods.
//!
//! Each method has distinct physics:
//! - Ion Beam Shepherd: Direct thrust from ion exhaust
//! - Gravity Tractor: Gravitational pull from spacecraft mass
//! - Laser Ablation: Momentum from vaporized surface material

use bevy::math::DVec2;

/// Gravitational constant (m³·kg⁻¹·s⁻²)
const G: f64 = 6.67430e-11;

/// Standard gravity (m/s²) for Isp calculations
const G0: f64 = 9.80665;

/// Ion Beam Shepherd thrust calculation.
///
/// The spacecraft hovers near the asteroid (50-500m) and directs its ion
/// exhaust at the asteroid surface, directly transferring momentum.
///
/// # Arguments
/// * `thrust_n` - Ion engine thrust in Newtons (typically 10 mN - 1 N)
/// * `asteroid_mass_kg` - Mass of the asteroid in kg
///
/// # Returns
/// Acceleration magnitude in m/s²
///
/// # Reference
/// Bombardelli, C. et al. (2011) "Ion Beam Shepherd for Asteroid Deflection"
/// https://arxiv.org/abs/1102.1276
#[inline]
pub fn ion_beam_acceleration(thrust_n: f64, asteroid_mass_kg: f64) -> f64 {
    if asteroid_mass_kg <= 0.0 {
        return 0.0;
    }
    // F = ma → a = F/m
    thrust_n / asteroid_mass_kg
}

/// Ion beam fuel consumption rate.
///
/// # Arguments
/// * `thrust_n` - Thrust in Newtons
/// * `specific_impulse` - Isp in seconds (typically 3000-5000 s for ion engines)
///
/// # Returns
/// Mass flow rate in kg/s
#[inline]
pub fn ion_fuel_consumption_rate(thrust_n: f64, specific_impulse: f64) -> f64 {
    if specific_impulse <= 0.0 {
        return 0.0;
    }
    // mdot = F / (Isp × g0)
    thrust_n / (specific_impulse * G0)
}

/// Gravity Tractor acceleration calculation.
///
/// The spacecraft hovers at a fixed distance from the asteroid, using its
/// thrusters to maintain position. The gravitational attraction between
/// the spacecraft and asteroid slowly accelerates the asteroid.
///
/// # Arguments
/// * `spacecraft_mass_kg` - Mass of the spacecraft in kg
/// * `hover_distance_m` - Distance from asteroid center of mass in meters
///
/// # Returns
/// Acceleration magnitude in m/s²
///
/// # Reference
/// Lu, E. & Love, S. (2005) "Gravitational tractor for towing asteroids"
/// 20,000 kg spacecraft at 200m → ~0.032 N force
#[inline]
pub fn gravity_tractor_acceleration(spacecraft_mass_kg: f64, hover_distance_m: f64) -> f64 {
    if hover_distance_m <= 0.0 {
        return 0.0;
    }
    // F = G × M × m / r²
    // a = F / m_asteroid = G × M_spacecraft / r²
    // (Note: this is the acceleration imparted TO the asteroid)
    G * spacecraft_mass_kg / (hover_distance_m * hover_distance_m)
}

/// Laser ablation thrust calculation.
///
/// A high-powered laser vaporizes the asteroid's surface, creating a plume
/// of gas/dust that imparts thrust to the asteroid.
///
/// # Arguments
/// * `power_kw` - Laser power in kilowatts
/// * `solar_distance_au` - Distance from Sun in AU (affects solar panel efficiency)
///
/// # Returns
/// Thrust in Newtons
///
/// # Reference
/// DE-STARLITE concept: 100 kW laser → 2.3 N thrust at 1 AU
/// Lubin, P. (2016) "A Roadmap to Interstellar Flight"
#[inline]
pub fn laser_ablation_thrust(power_kw: f64, solar_distance_au: f64) -> f64 {
    if power_kw <= 0.0 || solar_distance_au <= 0.0 {
        return 0.0;
    }

    // Base thrust: 2.3 N per 100 kW at 1 AU (DE-STARLITE reference)
    let base_thrust_per_100kw = 2.3;

    // Solar efficiency: power falls off with distance squared
    // (assuming solar-powered laser, not nuclear)
    let solar_efficiency = (1.0 / (solar_distance_au * solar_distance_au)).min(1.0);

    (power_kw / 100.0) * base_thrust_per_100kw * solar_efficiency
}

/// Laser ablation acceleration calculation.
///
/// # Arguments
/// * `power_kw` - Laser power in kilowatts
/// * `solar_distance_au` - Distance from Sun in AU
/// * `asteroid_mass_kg` - Mass of the asteroid in kg
///
/// # Returns
/// Acceleration magnitude in m/s²
#[inline]
pub fn laser_ablation_acceleration(
    power_kw: f64,
    solar_distance_au: f64,
    asteroid_mass_kg: f64,
) -> f64 {
    if asteroid_mass_kg <= 0.0 {
        return 0.0;
    }
    let thrust = laser_ablation_thrust(power_kw, solar_distance_au);
    thrust / asteroid_mass_kg
}

/// Solar sail thrust calculation.
///
/// A large reflective sail attached to the asteroid uses solar radiation
/// pressure to slowly push the asteroid. The thrust is proportional to
/// sail area and inversely proportional to distance from Sun squared.
///
/// # Arguments
/// * `sail_area_m2` - Sail area in square meters
/// * `solar_distance_au` - Distance from Sun in AU
///
/// # Returns
/// Thrust in Newtons
///
/// # Physics
/// Solar radiation pressure at 1 AU ≈ 9.08 μN/m² for perfect reflection.
/// P = 2 × S / c where S = 1361 W/m² (solar constant), c = 3×10⁸ m/s
/// Thrust = P × Area × (1 AU / distance)²
///
/// # Reference
/// NASA Solar Cruiser: 1,672 m² sail
/// Typical sails: 1,000 - 100,000 m²
#[inline]
pub fn solar_sail_thrust(sail_area_m2: f64, solar_distance_au: f64) -> f64 {
    if sail_area_m2 <= 0.0 || solar_distance_au <= 0.0 {
        return 0.0;
    }

    // Solar radiation pressure for perfect reflection at 1 AU
    // P = 2 × S / c = 2 × 1361 / 3e8 ≈ 9.08 μN/m²
    const SRP_AT_1AU: f64 = 9.08e-6; // N/m²

    // Thrust falls off with distance squared
    let distance_factor = 1.0 / (solar_distance_au * solar_distance_au);

    SRP_AT_1AU * sail_area_m2 * distance_factor
}

/// Solar sail acceleration calculation.
///
/// # Arguments
/// * `sail_area_m2` - Sail area in square meters
/// * `solar_distance_au` - Distance from Sun in AU
/// * `asteroid_mass_kg` - Mass of the asteroid in kg
///
/// # Returns
/// Acceleration magnitude in m/s²
#[inline]
pub fn solar_sail_acceleration(
    sail_area_m2: f64,
    solar_distance_au: f64,
    asteroid_mass_kg: f64,
) -> f64 {
    if asteroid_mass_kg <= 0.0 {
        return 0.0;
    }
    let thrust = solar_sail_thrust(sail_area_m2, solar_distance_au);
    thrust / asteroid_mass_kg
}

/// Compute thrust direction from reference direction.
///
/// # Arguments
/// * `asteroid_vel` - Asteroid velocity vector (m/s)
/// * `asteroid_pos` - Asteroid position vector (m)
/// * `direction` - Desired thrust direction type
///
/// # Returns
/// Unit vector in the thrust direction
pub fn compute_thrust_direction(
    asteroid_vel: DVec2,
    asteroid_pos: DVec2,
    direction: ThrustDirection,
) -> DVec2 {
    match direction {
        ThrustDirection::Retrograde => {
            let speed = asteroid_vel.length();
            if speed > 1e-6 {
                -asteroid_vel / speed
            } else {
                DVec2::ZERO
            }
        }
        ThrustDirection::Prograde => {
            let speed = asteroid_vel.length();
            if speed > 1e-6 {
                asteroid_vel / speed
            } else {
                DVec2::ZERO
            }
        }
        ThrustDirection::Radial => {
            // Perpendicular to velocity, in the orbital plane
            // For 2D, this is simply rotating velocity by 90 degrees
            let speed = asteroid_vel.length();
            if speed > 1e-6 {
                DVec2::new(-asteroid_vel.y, asteroid_vel.x) / speed
            } else {
                DVec2::ZERO
            }
        }
        ThrustDirection::AntiRadial => {
            // Opposite of radial
            let speed = asteroid_vel.length();
            if speed > 1e-6 {
                DVec2::new(asteroid_vel.y, -asteroid_vel.x) / speed
            } else {
                DVec2::ZERO
            }
        }
        ThrustDirection::SunPointing => {
            // Point thrust away from Sun (asteroid is pushed outward)
            let r = asteroid_pos.length();
            if r > 1e-6 {
                asteroid_pos / r
            } else {
                DVec2::ZERO
            }
        }
        ThrustDirection::Custom(dir) => {
            let len = dir.length();
            if len > 1e-6 {
                dir / len
            } else {
                DVec2::ZERO
            }
        }
    }
}

/// Thrust direction options for continuous deflection.
#[derive(Clone, Copy, Debug, PartialEq)]
#[derive(Default)]
pub enum ThrustDirection {
    /// Opposite to velocity (slows asteroid down - default for deflection)
    #[default]
    Retrograde,
    /// Same as velocity (speeds asteroid up)
    Prograde,
    /// Perpendicular to velocity, pointing inward
    Radial,
    /// Perpendicular to velocity, pointing outward
    AntiRadial,
    /// Pointing away from the Sun
    SunPointing,
    /// User-specified direction (unit vector)
    Custom(DVec2),
}


#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_ion_beam_acceleration() {
        // 100 mN thrust on 1e10 kg asteroid
        let thrust = 0.1; // 100 mN
        let mass = 1e10; // 10 billion kg
        let acc = ion_beam_acceleration(thrust, mass);
        assert!((acc - 1e-11).abs() < EPSILON);
    }

    #[test]
    fn test_ion_beam_zero_mass() {
        assert_eq!(ion_beam_acceleration(0.1, 0.0), 0.0);
    }

    #[test]
    fn test_ion_fuel_consumption() {
        // 100 mN at Isp = 3000 s
        let thrust = 0.1;
        let isp = 3000.0;
        let mdot = ion_fuel_consumption_rate(thrust, isp);
        // mdot = 0.1 / (3000 × 9.80665) ≈ 3.4e-6 kg/s
        let expected = 0.1 / (3000.0 * G0);
        assert!((mdot - expected).abs() < 1e-15);
    }

    #[test]
    fn test_gravity_tractor_acceleration() {
        // 20,000 kg at 200m
        let mass = 20_000.0;
        let distance = 200.0;
        let acc = gravity_tractor_acceleration(mass, distance);
        // a = G × 20000 / 200² = 6.67430e-11 × 20000 / 40000
        // ≈ 3.337e-11 m/s²
        let expected = G * mass / (distance * distance);
        assert!((acc - expected).abs() < 1e-20);
    }

    #[test]
    fn test_gravity_tractor_zero_distance() {
        assert_eq!(gravity_tractor_acceleration(20_000.0, 0.0), 0.0);
    }

    #[test]
    fn test_laser_ablation_thrust_at_1au() {
        // 100 kW at 1 AU should give 2.3 N
        let thrust = laser_ablation_thrust(100.0, 1.0);
        assert!((thrust - 2.3).abs() < 0.01);
    }

    #[test]
    fn test_laser_ablation_thrust_at_2au() {
        // At 2 AU, solar efficiency is 1/4
        let thrust = laser_ablation_thrust(100.0, 2.0);
        assert!((thrust - 2.3 / 4.0).abs() < 0.01);
    }

    #[test]
    fn test_laser_ablation_scales_with_power() {
        // 200 kW at 1 AU should give 4.6 N
        let thrust = laser_ablation_thrust(200.0, 1.0);
        assert!((thrust - 4.6).abs() < 0.01);
    }

    #[test]
    fn test_thrust_direction_retrograde() {
        let vel = DVec2::new(1000.0, 0.0);
        let pos = DVec2::new(1e11, 0.0);
        let dir = compute_thrust_direction(vel, pos, ThrustDirection::Retrograde);
        assert!((dir.x - (-1.0)).abs() < EPSILON);
        assert!(dir.y.abs() < EPSILON);
    }

    #[test]
    fn test_thrust_direction_prograde() {
        let vel = DVec2::new(1000.0, 0.0);
        let pos = DVec2::new(1e11, 0.0);
        let dir = compute_thrust_direction(vel, pos, ThrustDirection::Prograde);
        assert!((dir.x - 1.0).abs() < EPSILON);
        assert!(dir.y.abs() < EPSILON);
    }

    #[test]
    fn test_thrust_direction_radial() {
        let vel = DVec2::new(1000.0, 0.0);
        let pos = DVec2::new(1e11, 0.0);
        let dir = compute_thrust_direction(vel, pos, ThrustDirection::Radial);
        // Radial should be perpendicular to velocity (90° counterclockwise)
        // For vel=(1000, 0), radial is (0, 1000) normalized = (0, 1)
        assert!(dir.x.abs() < EPSILON);
        assert!((dir.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_thrust_direction_sun_pointing() {
        let vel = DVec2::new(1000.0, 0.0);
        let pos = DVec2::new(1e11, 0.0);
        let dir = compute_thrust_direction(vel, pos, ThrustDirection::SunPointing);
        assert!((dir.x - 1.0).abs() < EPSILON);
        assert!(dir.y.abs() < EPSILON);
    }

    #[test]
    fn test_thrust_direction_custom() {
        let vel = DVec2::new(1000.0, 0.0);
        let pos = DVec2::new(1e11, 0.0);
        let custom = DVec2::new(1.0, 1.0);
        let dir = compute_thrust_direction(vel, pos, ThrustDirection::Custom(custom));
        let expected = custom.normalize();
        assert!((dir.x - expected.x).abs() < EPSILON);
        assert!((dir.y - expected.y).abs() < EPSILON);
    }
}
