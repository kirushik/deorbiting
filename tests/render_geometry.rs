//! Render geometry tests for beam occlusion calculations.
//!
//! Tests pure geometry functions extracted from render/deflectors.rs.

use bevy::math::Vec3;

// ============================================================================
// Line-circle intersection (for beam occlusion)
// ============================================================================

/// Result of a line-circle intersection test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineCircleIntersection {
    /// Line does not intersect the circle.
    NoIntersection,
    /// Line intersects at one point (tangent) - parametric distance along line.
    Tangent(f32),
    /// Line intersects at two points - entry and exit parametric distances.
    TwoPoints { entry: f32, exit: f32 },
}

/// Calculate line-circle intersection using parametric line representation.
///
/// Line is defined as: P(t) = line_start + t * line_dir
/// where t in [0, line_length] represents the beam segment.
///
/// Returns the parametric t values where intersection occurs.
pub fn line_circle_intersection(
    line_start: Vec3,
    line_dir: Vec3,
    line_length: f32,
    circle_center: Vec3,
    circle_radius: f32,
) -> LineCircleIntersection {
    // Vector from line start to circle center
    let to_center = circle_center - line_start;

    // Project center onto line (using 2D - ignoring Z)
    let to_center_2d = Vec3::new(to_center.x, to_center.y, 0.0);
    let line_dir_2d = Vec3::new(line_dir.x, line_dir.y, 0.0);

    // t_closest is the parametric position of the closest point on line to circle center
    let t_closest = to_center_2d.dot(line_dir_2d);

    // Skip if circle is entirely behind line start or past line end
    if t_closest < -circle_radius || t_closest > line_length + circle_radius {
        return LineCircleIntersection::NoIntersection;
    }

    // Distance from circle center to closest point on line
    let closest_point = line_start + line_dir * t_closest;
    let dist_to_line = (circle_center - closest_point).length();

    // No intersection if line passes outside circle
    if dist_to_line > circle_radius {
        return LineCircleIntersection::NoIntersection;
    }

    // Tangent case (line barely touches circle)
    if (dist_to_line - circle_radius).abs() < 1e-6 {
        // Only count if tangent point is within segment
        if t_closest >= 0.0 && t_closest <= line_length {
            return LineCircleIntersection::Tangent(t_closest);
        }
        return LineCircleIntersection::NoIntersection;
    }

    // Two intersection points
    let discriminant = circle_radius * circle_radius - dist_to_line * dist_to_line;
    let half_chord = discriminant.sqrt();

    let t_entry = t_closest - half_chord;
    let t_exit = t_closest + half_chord;

    // Check if intersection segment overlaps with line segment [0, line_length]
    if t_exit < 0.0 || t_entry > line_length {
        return LineCircleIntersection::NoIntersection;
    }

    LineCircleIntersection::TwoPoints {
        entry: t_entry.max(0.0),
        exit: t_exit.min(line_length),
    }
}

/// Calculate the effective beam endpoint considering occlusion.
///
/// Given a beam from start to end, check if any occluder blocks it.
/// Returns the effective endpoint (either the original or where it's blocked).
///
/// occluders: list of (center, radius) pairs.
/// skip_near_start_distance: bodies within this distance of start are ignored
///                          (e.g., Earth shouldn't block its own beam).
pub fn calculate_effective_beam_endpoint(
    beam_start: Vec3,
    beam_end: Vec3,
    occluders: &[(Vec3, f32)],
    skip_near_start_distance: f32,
) -> (Vec3, bool) {
    let beam_vec = beam_end - beam_start;
    let beam_length = beam_vec.length();
    if beam_length < 0.01 {
        return (beam_end, false);
    }
    let beam_dir = beam_vec / beam_length;

    let mut effective_end = beam_end;
    let mut is_occluded = false;

    for &(body_pos, body_radius) in occluders {
        // Skip bodies too close to beam start
        let dist_to_start = (body_pos - beam_start).length();
        if dist_to_start < skip_near_start_distance {
            continue;
        }

        match line_circle_intersection(beam_start, beam_dir, beam_length, body_pos, body_radius) {
            LineCircleIntersection::TwoPoints { entry, .. } => {
                // Update effective endpoint if this occlusion is earlier
                let occlusion_point = beam_start + beam_dir * entry;
                let current_dist = (effective_end - beam_start).length();
                if entry < current_dist {
                    effective_end = occlusion_point;
                    is_occluded = true;
                }
            }
            LineCircleIntersection::Tangent(t) => {
                // Tangent still counts as occlusion
                let occlusion_point = beam_start + beam_dir * t;
                let current_dist = (effective_end - beam_start).length();
                if t < current_dist {
                    effective_end = occlusion_point;
                    is_occluded = true;
                }
            }
            LineCircleIntersection::NoIntersection => {}
        }
    }

    (effective_end, is_occluded)
}

// ============================================================================
// Line-circle intersection tests
// ============================================================================

#[test]
fn test_line_circle_no_intersection() {
    let result = line_circle_intersection(
        Vec3::new(0.0, 0.0, 0.0),   // line start
        Vec3::new(1.0, 0.0, 0.0),   // line direction (right)
        100.0,                      // line length
        Vec3::new(50.0, 20.0, 0.0), // circle center (above line)
        10.0,                       // circle radius (not reaching line)
    );
    assert_eq!(result, LineCircleIntersection::NoIntersection);
}

#[test]
fn test_line_circle_direct_hit() {
    // Line passes through center of circle
    let result = line_circle_intersection(
        Vec3::new(0.0, 0.0, 0.0), // line start
        Vec3::new(1.0, 0.0, 0.0), // line direction
        100.0,                    // line length
        Vec3::new(50.0, 0.0, 0.0), // circle center on line
        10.0,                     // circle radius
    );

    match result {
        LineCircleIntersection::TwoPoints { entry, exit } => {
            assert!((entry - 40.0).abs() < 0.01); // 50 - 10
            assert!((exit - 60.0).abs() < 0.01); // 50 + 10
        }
        _ => panic!("Expected TwoPoints, got {:?}", result),
    }
}

#[test]
fn test_line_circle_tangent() {
    // Line tangent to circle
    let result = line_circle_intersection(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        100.0,
        Vec3::new(50.0, 10.0, 0.0), // circle center 10 units above line
        10.0,                       // radius exactly touches line
    );

    match result {
        LineCircleIntersection::Tangent(t) => {
            assert!((t - 50.0).abs() < 0.01);
        }
        _ => panic!("Expected Tangent, got {:?}", result),
    }
}

#[test]
fn test_line_circle_behind_start() {
    // Circle entirely behind line start
    let result = line_circle_intersection(
        Vec3::new(100.0, 0.0, 0.0), // start at x=100
        Vec3::new(1.0, 0.0, 0.0),   // going right
        100.0,
        Vec3::new(50.0, 0.0, 0.0), // circle at x=50 (behind start)
        10.0,
    );
    assert_eq!(result, LineCircleIntersection::NoIntersection);
}

#[test]
fn test_line_circle_past_end() {
    // Circle entirely past line end
    let result = line_circle_intersection(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        50.0, // line only goes to x=50
        Vec3::new(100.0, 0.0, 0.0), // circle at x=100
        10.0,
    );
    assert_eq!(result, LineCircleIntersection::NoIntersection);
}

#[test]
fn test_line_circle_partial_intersection_at_start() {
    // Line starts inside circle
    let result = line_circle_intersection(
        Vec3::new(0.0, 0.0, 0.0),   // start at origin
        Vec3::new(1.0, 0.0, 0.0),   // going right
        100.0,                      // long line
        Vec3::new(-5.0, 0.0, 0.0), // circle centered behind start
        10.0,                       // radius extends past start
    );

    match result {
        LineCircleIntersection::TwoPoints { entry, exit } => {
            assert_eq!(entry, 0.0); // clamped to 0
            assert!((exit - 5.0).abs() < 0.01); // -5 + 10
        }
        _ => panic!("Expected TwoPoints with entry clamped to 0"),
    }
}

#[test]
fn test_line_circle_diagonal() {
    // Diagonal line hitting circle
    let dir = Vec3::new(1.0, 1.0, 0.0).normalize();
    let result = line_circle_intersection(
        Vec3::new(0.0, 0.0, 0.0),
        dir,
        200.0,
        Vec3::new(50.0, 50.0, 0.0), // on the diagonal
        10.0,
    );

    match result {
        LineCircleIntersection::TwoPoints { entry, exit } => {
            // Distance to (50, 50) along diagonal is sqrt(50² + 50²) = 70.7
            let expected_dist = (50.0_f32.powi(2) + 50.0_f32.powi(2)).sqrt();
            assert!((entry - (expected_dist - 10.0)).abs() < 0.1);
            assert!((exit - (expected_dist + 10.0)).abs() < 0.1);
        }
        _ => panic!("Expected TwoPoints"),
    }
}

// ============================================================================
// Beam endpoint calculation tests
// ============================================================================

#[test]
fn test_beam_endpoint_no_occluders() {
    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        &[],
        20.0,
    );

    assert_eq!(endpoint, Vec3::new(100.0, 0.0, 0.0));
    assert!(!occluded);
}

#[test]
fn test_beam_endpoint_occluder_in_path() {
    let occluders = vec![(Vec3::new(50.0, 0.0, 0.0), 10.0)];

    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        &occluders,
        20.0,
    );

    // Beam should stop at entry to circle (50 - 10 = 40)
    assert!((endpoint.x - 40.0).abs() < 0.01);
    assert!((endpoint.y - 0.0).abs() < 0.01);
    assert!(occluded);
}

#[test]
fn test_beam_endpoint_skip_near_start() {
    // Occluder very close to start should be skipped (like Earth for its own beam)
    let occluders = vec![(Vec3::new(10.0, 0.0, 0.0), 15.0)];

    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        &occluders,
        30.0, // skip anything within 30 units of start
    );

    // Occluder at 10 units should be skipped
    assert_eq!(endpoint, Vec3::new(100.0, 0.0, 0.0));
    assert!(!occluded);
}

#[test]
fn test_beam_endpoint_multiple_occluders_first_wins() {
    let occluders = vec![
        (Vec3::new(70.0, 0.0, 0.0), 10.0), // farther, at x=60-80
        (Vec3::new(50.0, 0.0, 0.0), 10.0), // closer, at x=40-60
    ];

    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        &occluders,
        20.0,
    );

    // Closer occluder should win (entry at 40)
    assert!((endpoint.x - 40.0).abs() < 0.01);
    assert!(occluded);
}

#[test]
fn test_beam_endpoint_occluder_off_path() {
    let occluders = vec![(Vec3::new(50.0, 50.0, 0.0), 10.0)]; // above beam path

    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        &occluders,
        20.0,
    );

    // Occluder doesn't block beam
    assert_eq!(endpoint, Vec3::new(100.0, 0.0, 0.0));
    assert!(!occluded);
}

#[test]
fn test_beam_endpoint_zero_length_beam() {
    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(50.0, 50.0, 0.0), // same point
        &[(Vec3::new(50.0, 50.0, 0.0), 10.0)],
        20.0,
    );

    // Zero-length beam returns end point without checking occlusion
    assert_eq!(endpoint, Vec3::new(50.0, 50.0, 0.0));
    assert!(!occluded);
}

#[test]
fn test_beam_endpoint_diagonal_occlusion() {
    // Beam going diagonally, occluder in the path
    let occluders = vec![(Vec3::new(50.0, 50.0, 0.0), 10.0)];

    let (endpoint, occluded) = calculate_effective_beam_endpoint(
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 100.0, 0.0),
        &occluders,
        20.0,
    );

    assert!(occluded);
    // Entry should be before the circle center
    let dist_to_endpoint = (endpoint - Vec3::new(0.0, 0.0, 0.0)).length();
    let dist_to_center = (50.0_f32.powi(2) + 50.0_f32.powi(2)).sqrt();
    assert!(dist_to_endpoint < dist_to_center);
}
