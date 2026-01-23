//! Lambert's problem solver using universal variable formulation.
//!
//! Lambert's problem: given two position vectors and time of flight,
//! find the orbit connecting them.
//!
//! This solver uses the Battin algorithm which handles all orbit types
//! (elliptical, parabolic, hyperbolic) robustly.

use bevy::math::DVec2;

/// Stumpff function C(z) - handles elliptic/parabolic/hyperbolic cases.
fn stumpff_c(z: f64) -> f64 {
    if z > 1e-4 {
        // Elliptic
        let sqrt_z = z.sqrt();
        (1.0 - sqrt_z.cos()) / z
    } else if z < -1e-4 {
        // Hyperbolic
        let sqrt_neg_z = (-z).sqrt();
        (sqrt_neg_z.cosh() - 1.0) / (-z)
    } else {
        // Parabolic limit: Taylor expansion for numerical stability
        1.0 / 2.0 - z / 24.0 + z * z / 720.0 - z * z * z / 40320.0
    }
}

/// Stumpff function S(z) - handles elliptic/parabolic/hyperbolic cases.
fn stumpff_s(z: f64) -> f64 {
    if z > 1e-4 {
        // Elliptic
        let sqrt_z = z.sqrt();
        (sqrt_z - sqrt_z.sin()) / (sqrt_z.powi(3))
    } else if z < -1e-4 {
        // Hyperbolic
        let sqrt_neg_z = (-z).sqrt();
        (sqrt_neg_z.sinh() - sqrt_neg_z) / sqrt_neg_z.powi(3)
    } else {
        // Parabolic limit: Taylor expansion
        1.0 / 6.0 - z / 120.0 + z * z / 5040.0 - z * z * z / 362880.0
    }
}

/// Result of Lambert solver.
#[derive(Debug, Clone, Copy)]
pub struct LambertSolution {
    /// Departure velocity (at r1)
    pub v1: DVec2,
    /// Arrival velocity (at r2)
    pub v2: DVec2,
    /// Semi-major axis of transfer orbit (negative for hyperbolic)
    pub semi_major_axis: f64,
}

/// Solve Lambert's problem using universal variable formulation.
///
/// Given two position vectors and time of flight, find the orbit connecting them.
///
/// # Arguments
/// * `r1` - Initial position vector (meters)
/// * `r2` - Final position vector (meters)
/// * `tof` - Time of flight (seconds)
/// * `mu` - Gravitational parameter GM (m³/s²)
/// * `prograde` - If true, use prograde (short way) solution; if false, retrograde
///
/// # Returns
/// * `Some(LambertSolution)` - Solution with departure/arrival velocities if found
/// * `None` - If no solution (iteration didn't converge)
pub fn solve_lambert(
    r1: DVec2,
    r2: DVec2,
    tof: f64,
    mu: f64,
    prograde: bool,
) -> Option<LambertSolution> {
    let r1_mag = r1.length();
    let r2_mag = r2.length();

    if r1_mag < 1e-6 || r2_mag < 1e-6 || tof < 1e-6 {
        return None; // Degenerate case
    }

    // Transfer angle calculation using 2D cross product (z-component)
    let cross_z = r1.x * r2.y - r1.y * r2.x; // Cross product z-component
    let cos_dnu = (r1.dot(r2) / (r1_mag * r2_mag)).clamp(-1.0, 1.0);

    // Determine transfer angle (0 to 2π)
    let sin_dnu = if prograde {
        // Short way: positive cross means counter-clockwise
        if cross_z >= 0.0 {
            (1.0 - cos_dnu * cos_dnu).sqrt()
        } else {
            -(1.0 - cos_dnu * cos_dnu).sqrt()
        }
    } else {
        // Long way: opposite
        if cross_z >= 0.0 {
            -(1.0 - cos_dnu * cos_dnu).sqrt()
        } else {
            (1.0 - cos_dnu * cos_dnu).sqrt()
        }
    };

    // Check for near-180° transfer (degenerate)
    if (1.0 + cos_dnu).abs() < 1e-10 {
        return None;
    }

    // A coefficient (chord parameter)
    let a_coeff = (r1_mag * r2_mag * (1.0 + cos_dnu)).sqrt();
    if sin_dnu < 0.0 {
        // Long way transfer
    }
    let a_coeff = if sin_dnu >= 0.0 { a_coeff } else { -a_coeff };

    // Initial guess for psi using parabolic time of flight
    let chord = (r2 - r1).length();
    let s = (r1_mag + r2_mag + chord) / 2.0; // Semi-perimeter
    let t_parabolic = (2.0 / 3.0) * (s.powi(3) / mu).sqrt() * (1.0 - ((s - chord) / s).powf(1.5));

    // Use bisection to find psi
    let mut psi_low: f64;
    let mut psi_high: f64;

    if tof < t_parabolic {
        // Hyperbolic: psi < 0
        psi_low = -4.0 * std::f64::consts::PI * std::f64::consts::PI;
        psi_high = 0.0;
    } else {
        // Elliptic: psi > 0
        psi_low = 0.0;
        psi_high = 4.0 * std::f64::consts::PI * std::f64::consts::PI;
    }

    const MAX_ITER: usize = 50;
    const TOL: f64 = 1e-8;

    let mut psi = (psi_low + psi_high) / 2.0;

    for _ in 0..MAX_ITER {
        let c = stumpff_c(psi);
        let s_stumpff = stumpff_s(psi);

        // y must be positive for real solution
        let y = r1_mag + r2_mag + a_coeff * (psi * s_stumpff - 1.0) / c.sqrt();

        if c.abs() < 1e-12 || y < 0.0 {
            // Adjust bounds
            if sin_dnu >= 0.0 {
                psi_low = psi;
            } else {
                psi_high = psi;
            }
            psi = (psi_low + psi_high) / 2.0;
            continue;
        }

        let chi = (y / c).sqrt();
        let chi3 = chi.powi(3);

        // Time of flight equation
        let tof_calc = (chi3 * s_stumpff + a_coeff * y.sqrt()) / mu.sqrt();

        let dt = tof - tof_calc;

        if dt.abs() < TOL * tof {
            // Converged! Compute f, g, f_dot, g_dot
            let f = 1.0 - y / r1_mag;
            let g = a_coeff * (y / mu).sqrt();
            let g_dot = 1.0 - y / r2_mag;

            if g.abs() < 1e-12 {
                return None; // Degenerate
            }

            let v1 = (r2 - r1 * f) / g;
            let v2 = (r2 * g_dot - r1) / g;

            // Semi-major axis from vis-viva
            let energy = v1.length_squared() / 2.0 - mu / r1_mag;
            let semi_major = if energy.abs() > 1e-12 {
                -mu / (2.0 * energy)
            } else {
                f64::INFINITY // Parabolic
            };

            return Some(LambertSolution {
                v1,
                v2,
                semi_major_axis: semi_major,
            });
        }

        // Bisection update
        if dt > 0.0 {
            psi_low = psi;
        } else {
            psi_high = psi;
        }
        psi = (psi_low + psi_high) / 2.0;
    }

    None // Didn't converge
}

/// Solve Lambert's problem with automatic direction selection.
///
/// Tries both prograde and retrograde, returns the one with lower departure velocity.
pub fn solve_lambert_auto(r1: DVec2, r2: DVec2, tof: f64, mu: f64) -> Option<LambertSolution> {
    let prograde = solve_lambert(r1, r2, tof, mu, true);
    let retrograde = solve_lambert(r1, r2, tof, mu, false);

    match (prograde, retrograde) {
        (Some(p), Some(r)) => {
            if p.v1.length() <= r.v1.length() {
                Some(p)
            } else {
                Some(r)
            }
        }
        (Some(p), None) => Some(p),
        (None, Some(r)) => Some(r),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::GM_SUN;

    const AU: f64 = 1.495978707e11;

    #[test]
    fn test_stumpff_c_limits() {
        let c_neg = stumpff_c(-0.01);
        let c_zero = stumpff_c(0.0);
        let c_pos = stumpff_c(0.01);

        assert!((c_neg - c_zero).abs() < 0.01, "C continuity at z=0");
        assert!((c_pos - c_zero).abs() < 0.01, "C continuity at z=0");
        assert!((c_zero - 0.5).abs() < 0.01, "C(0) ≈ 0.5");
    }

    #[test]
    fn test_stumpff_s_limits() {
        let s_neg = stumpff_s(-0.01);
        let s_zero = stumpff_s(0.0);
        let s_pos = stumpff_s(0.01);

        assert!((s_neg - s_zero).abs() < 0.01, "S continuity at z=0");
        assert!((s_pos - s_zero).abs() < 0.01, "S continuity at z=0");
        assert!((s_zero - 1.0 / 6.0).abs() < 0.01, "S(0) ≈ 1/6");
    }

    #[test]
    fn test_circular_orbit_quarter() {
        // Transfer from (1 AU, 0) to (0, 1 AU) - 90° around circular orbit
        let r1 = DVec2::new(AU, 0.0);
        let r2 = DVec2::new(0.0, AU);

        // Quarter period at 1 AU ≈ 91.3 days
        let period = 2.0 * std::f64::consts::PI * (AU.powi(3) / GM_SUN).sqrt();
        let tof = period / 4.0;

        let solution = solve_lambert(r1, r2, tof, GM_SUN, true);
        assert!(solution.is_some(), "90° transfer should converge");

        let sol = solution.unwrap();
        // Circular orbit velocity at 1 AU ≈ 29.78 km/s
        let v_circular = (GM_SUN / AU).sqrt();
        let v1_mag = sol.v1.length();

        // Should be close to circular velocity
        assert!(
            (v1_mag - v_circular).abs() / v_circular < 0.1,
            "v1={:.1} km/s should be near circular {:.1} km/s",
            v1_mag / 1000.0,
            v_circular / 1000.0
        );
    }

    #[test]
    fn test_near_hohmann_earth_mars() {
        // Near-Hohmann transfer: Earth (1 AU) to Mars orbit (~1.524 AU)
        // Use slight offset from 180° since exact 180° is degenerate in 2D
        // This is realistic - real transfers rarely hit exactly 180°
        let r1 = DVec2::new(AU, 0.0);
        // 175° instead of 180°
        let angle = 175.0_f64.to_radians();
        let r2 = DVec2::new(1.524 * AU * angle.cos(), 1.524 * AU * angle.sin());

        // Approximate Hohmann transfer time
        let a_transfer = (1.0 + 1.524) / 2.0 * AU;
        let tof = 0.95 * std::f64::consts::PI * (a_transfer.powi(3) / GM_SUN).sqrt();

        let solution = solve_lambert(r1, r2, tof, GM_SUN, true);
        assert!(solution.is_some(), "Near-Hohmann transfer should converge");

        if let Some(sol) = solution {
            let v1_mag = sol.v1.length() / 1000.0;
            // Should be in reasonable range for interplanetary transfer
            assert!(
                v1_mag > 25.0 && v1_mag < 40.0,
                "Departure velocity {:.1} km/s should be reasonable",
                v1_mag
            );
        }
    }

    #[test]
    fn test_fast_transfer() {
        // Fast transfer - should be hyperbolic
        let r1 = DVec2::new(AU, 0.0);
        let r2 = DVec2::new(0.0, 1.2 * AU);

        // Very short time - forces hyperbolic
        let tof = 30.0 * 86400.0; // 30 days

        let solution = solve_lambert(r1, r2, tof, GM_SUN, true);
        assert!(solution.is_some(), "Fast transfer should converge");
    }

    #[test]
    fn test_auto_direction() {
        let r1 = DVec2::new(AU, 0.0);
        let r2 = DVec2::new(0.0, AU);
        let tof = 91.0 * 86400.0;

        let solution = solve_lambert_auto(r1, r2, tof, GM_SUN);
        assert!(solution.is_some(), "Auto direction should find solution");
    }
}
