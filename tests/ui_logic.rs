//! UI logic tests for box selection and card positioning.
//!
//! Tests pure logic functions that can be extracted from the UI systems.

use bevy::math::Vec2;

// ============================================================================
// BoxSelectionState tests (coordinate transformations)
// ============================================================================

/// Test data structure mirroring BoxSelectionState for testing rect calculations.
#[derive(Default, Clone)]
struct BoxSelectionState {
    start_screen: Vec2,
    current_screen: Vec2,
    start_world: Vec2,
    current_world: Vec2,
}

impl BoxSelectionState {
    fn screen_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(
            self.start_screen.x.min(self.current_screen.x),
            self.start_screen.y.min(self.current_screen.y),
        );
        let max = Vec2::new(
            self.start_screen.x.max(self.current_screen.x),
            self.start_screen.y.max(self.current_screen.y),
        );
        (min, max)
    }

    fn world_rect(&self) -> (Vec2, Vec2) {
        let min = Vec2::new(
            self.start_world.x.min(self.current_world.x),
            self.start_world.y.min(self.current_world.y),
        );
        let max = Vec2::new(
            self.start_world.x.max(self.current_world.x),
            self.start_world.y.max(self.current_world.y),
        );
        (min, max)
    }
}

#[test]
fn test_screen_rect_standard_drag() {
    // Drag from top-left to bottom-right
    let state = BoxSelectionState {
        start_screen: Vec2::new(100.0, 100.0),
        current_screen: Vec2::new(200.0, 200.0),
        ..Default::default()
    };
    let (min, max) = state.screen_rect();
    assert_eq!(min, Vec2::new(100.0, 100.0));
    assert_eq!(max, Vec2::new(200.0, 200.0));
}

#[test]
fn test_screen_rect_reverse_drag() {
    // Drag from bottom-right to top-left (reverse direction)
    let state = BoxSelectionState {
        start_screen: Vec2::new(200.0, 200.0),
        current_screen: Vec2::new(100.0, 100.0),
        ..Default::default()
    };
    let (min, max) = state.screen_rect();
    // Should still produce correct min/max regardless of drag direction
    assert_eq!(min, Vec2::new(100.0, 100.0));
    assert_eq!(max, Vec2::new(200.0, 200.0));
}

#[test]
fn test_screen_rect_diagonal_reverse() {
    // Drag from top-right to bottom-left
    let state = BoxSelectionState {
        start_screen: Vec2::new(300.0, 50.0),
        current_screen: Vec2::new(100.0, 150.0),
        ..Default::default()
    };
    let (min, max) = state.screen_rect();
    assert_eq!(min, Vec2::new(100.0, 50.0));
    assert_eq!(max, Vec2::new(300.0, 150.0));
}

#[test]
fn test_world_rect_matches_screen_rect_behavior() {
    // World coordinates should behave the same as screen coordinates
    let state = BoxSelectionState {
        start_world: Vec2::new(-100.0, 50.0),
        current_world: Vec2::new(200.0, -100.0),
        ..Default::default()
    };
    let (min, max) = state.world_rect();
    assert_eq!(min, Vec2::new(-100.0, -100.0));
    assert_eq!(max, Vec2::new(200.0, 50.0));
}

// ============================================================================
// Asteroid-in-region detection (extracted pure logic)
// ============================================================================

/// Check if a position is inside a rectangular region.
fn is_position_in_region(pos: Vec2, world_min: Vec2, world_max: Vec2) -> bool {
    pos.x >= world_min.x && pos.x <= world_max.x && pos.y >= world_min.y && pos.y <= world_max.y
}

/// Find asteroids in a box region and sort by distance to center.
/// Returns indices of asteroids sorted by distance to box center.
fn find_asteroids_in_region(
    asteroid_positions: &[Vec2],
    world_min: Vec2,
    world_max: Vec2,
) -> Vec<(usize, f32)> {
    let box_center = (world_min + world_max) / 2.0;

    let mut asteroids_in_box: Vec<(usize, f32)> = asteroid_positions
        .iter()
        .enumerate()
        .filter_map(|(idx, &pos)| {
            if is_position_in_region(pos, world_min, world_max) {
                let dist = (pos - box_center).length();
                if dist.is_finite() {
                    Some((idx, dist))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

    // Sort by distance to center
    asteroids_in_box.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    asteroids_in_box
}

#[test]
fn test_position_in_region_inside() {
    let pos = Vec2::new(50.0, 50.0);
    let min = Vec2::new(0.0, 0.0);
    let max = Vec2::new(100.0, 100.0);
    assert!(is_position_in_region(pos, min, max));
}

#[test]
fn test_position_in_region_on_boundary() {
    let min = Vec2::new(0.0, 0.0);
    let max = Vec2::new(100.0, 100.0);

    // Test all boundaries
    assert!(is_position_in_region(Vec2::new(0.0, 50.0), min, max)); // left edge
    assert!(is_position_in_region(Vec2::new(100.0, 50.0), min, max)); // right edge
    assert!(is_position_in_region(Vec2::new(50.0, 0.0), min, max)); // bottom edge
    assert!(is_position_in_region(Vec2::new(50.0, 100.0), min, max)); // top edge
    assert!(is_position_in_region(Vec2::new(0.0, 0.0), min, max)); // corner
}

#[test]
fn test_position_in_region_outside() {
    let min = Vec2::new(0.0, 0.0);
    let max = Vec2::new(100.0, 100.0);

    assert!(!is_position_in_region(Vec2::new(-1.0, 50.0), min, max)); // left of region
    assert!(!is_position_in_region(Vec2::new(101.0, 50.0), min, max)); // right of region
    assert!(!is_position_in_region(Vec2::new(50.0, -1.0), min, max)); // below region
    assert!(!is_position_in_region(Vec2::new(50.0, 101.0), min, max)); // above region
}

#[test]
fn test_find_asteroids_in_region_empty() {
    let positions: Vec<Vec2> = vec![];
    let result = find_asteroids_in_region(&positions, Vec2::ZERO, Vec2::new(100.0, 100.0));
    assert!(result.is_empty());
}

#[test]
fn test_find_asteroids_in_region_none_inside() {
    let positions = vec![
        Vec2::new(-50.0, 50.0),  // outside left
        Vec2::new(150.0, 50.0), // outside right
    ];
    let result = find_asteroids_in_region(&positions, Vec2::ZERO, Vec2::new(100.0, 100.0));
    assert!(result.is_empty());
}

#[test]
fn test_find_asteroids_in_region_one_inside() {
    let positions = vec![
        Vec2::new(-50.0, 50.0), // outside
        Vec2::new(50.0, 50.0),  // inside (center)
        Vec2::new(150.0, 50.0), // outside
    ];
    let result = find_asteroids_in_region(&positions, Vec2::ZERO, Vec2::new(100.0, 100.0));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 1); // index 1 is inside
}

#[test]
fn test_find_asteroids_in_region_sorted_by_distance() {
    let positions = vec![
        Vec2::new(80.0, 50.0), // inside, farther from center (50,50)
        Vec2::new(50.0, 50.0), // inside, at center
        Vec2::new(55.0, 55.0), // inside, near center
    ];
    let result = find_asteroids_in_region(&positions, Vec2::ZERO, Vec2::new(100.0, 100.0));
    assert_eq!(result.len(), 3);
    // Should be sorted by distance: center (idx 1) first, near center (idx 2) second, far (idx 0) last
    assert_eq!(result[0].0, 1); // exactly at center, distance = 0
    assert_eq!(result[1].0, 2); // near center, distance ~7.07
    assert_eq!(result[2].0, 0); // far from center, distance = 30
}

#[test]
fn test_find_asteroids_filters_nan_distance() {
    let positions = vec![
        Vec2::new(f32::NAN, 50.0), // NaN position
        Vec2::new(50.0, 50.0),     // valid position at center
    ];
    let result = find_asteroids_in_region(&positions, Vec2::ZERO, Vec2::new(100.0, 100.0));
    // NaN position won't match the region check (NaN comparisons are false)
    // and NaN distance would be filtered out anyway
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, 1);
}

// ============================================================================
// Card position scoring (extracted pure logic from smart_card_position)
// ============================================================================

/// Screen bounds for card positioning tests.
struct ScreenBounds {
    left: f32,
    right: f32,
    top: f32,
    usable_bottom: f32,
}

/// Calculate position score for a card placement candidate.
/// Returns a score where higher is better.
fn score_candidate_position(
    card_left: f32,
    card_top: f32,
    card_width: f32,
    card_height: f32,
    screen: &ScreenBounds,
    velocity_dir: Option<Vec2>,
    object_screen_pos: Vec2,
    velocity_arrow_length: f32,
    offset_x: f32,
) -> f32 {
    let card_right = card_left + card_width;
    let card_bottom = card_top + card_height;

    let mut score: f32 = 100.0;

    // Penalty: off-screen left
    if card_left < screen.left {
        score -= (screen.left - card_left) * 2.0;
    }
    // Penalty: off-screen right
    if card_right > screen.right {
        score -= (card_right - screen.right) * 2.0;
    }
    // Penalty: off-screen top
    if card_top < screen.top {
        score -= (screen.top - card_top) * 2.0;
    }
    // Penalty: overlaps dock area
    if card_bottom > screen.usable_bottom {
        score -= (card_bottom - screen.usable_bottom) * 3.0;
    }

    // Penalty: overlaps velocity arrow zone
    if let Some(vel_dir) = velocity_dir {
        let arrow_end_x = object_screen_pos.x + vel_dir.x * velocity_arrow_length;
        let arrow_end_y = object_screen_pos.y + vel_dir.y * velocity_arrow_length;
        let arrow_mid_x = object_screen_pos.x + vel_dir.x * velocity_arrow_length * 0.5;
        let arrow_mid_y = object_screen_pos.y + vel_dir.y * velocity_arrow_length * 0.5;

        let endpoint_inside = arrow_end_x >= card_left
            && arrow_end_x <= card_right
            && arrow_end_y >= card_top
            && arrow_end_y <= card_bottom;
        let midpoint_inside = arrow_mid_x >= card_left
            && arrow_mid_x <= card_right
            && arrow_mid_y >= card_top
            && arrow_mid_y <= card_bottom;

        if endpoint_inside {
            score -= 80.0; // Heavy penalty - this blocks the drag handle
        }
        if midpoint_inside {
            score -= 40.0; // Medium penalty
        }

        // Bonus: position is on opposite side from velocity
        let card_center_x = card_left + card_width / 2.0;
        let card_center_y = card_top + card_height / 2.0;
        let to_card_x = card_center_x - object_screen_pos.x;
        let to_card_y = card_center_y - object_screen_pos.y;
        let dot = to_card_x * vel_dir.x + to_card_y * vel_dir.y;
        if dot < 0.0 {
            score += 20.0; // Bonus for being on opposite side
        }
    }

    // Slight preference for right side (reading direction)
    if offset_x > 0.0 {
        score += 5.0;
    }

    score
}

/// Clamp card position to screen bounds.
fn clamp_card_to_screen(
    pos_x: f32,
    pos_y: f32,
    card_width: f32,
    card_height: f32,
    screen: &ScreenBounds,
) -> (f32, f32) {
    let final_x = pos_x.clamp(screen.left + 5.0, screen.right - card_width - 5.0);
    let final_y = pos_y.clamp(screen.top + 5.0, screen.usable_bottom - card_height);
    (final_x, final_y)
}

#[test]
fn test_score_position_fully_visible() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0, // dock at 600, usable ends at 550
    };

    // Card fully within screen, on right side
    let score = score_candidate_position(
        200.0, // card_left
        200.0, // card_top
        150.0, // card_width
        100.0, // card_height
        &screen,
        None,                    // no velocity
        Vec2::new(180.0, 250.0), // object position
        80.0,                    // arrow length
        20.0,                    // positive offset (right side)
    );

    // Should get base score (100) + right side bonus (5)
    assert!((score - 105.0).abs() < 0.01);
}

#[test]
fn test_score_position_off_screen_left() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let score = score_candidate_position(
        -50.0, // card_left - 50px off screen
        200.0,
        150.0,
        100.0,
        &screen,
        None,
        Vec2::new(100.0, 250.0),
        80.0,
        -200.0, // left side
    );

    // Should have penalty: 50 * 2 = 100 points off, so score = 100 - 100 = 0
    assert!((score - 0.0).abs() < 0.01);
}

#[test]
fn test_score_position_overlaps_dock() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let score = score_candidate_position(
        200.0, // card_left
        520.0, // card_top - bottom edge at 620, 70px into dock
        150.0,
        100.0,
        &screen,
        None,
        Vec2::new(180.0, 500.0),
        80.0,
        20.0,
    );

    // card_bottom = 520 + 100 = 620, overlap = 620 - 550 = 70
    // Penalty: 70 * 3 = 210
    // Score: 100 + 5 (right) - 210 = -105
    assert!((score - (-105.0)).abs() < 0.01);
}

#[test]
fn test_score_position_velocity_arrow_overlap() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let object_pos = Vec2::new(200.0, 200.0);
    let vel_dir = Vec2::new(1.0, 0.0); // velocity pointing right
    let arrow_length = 80.0;
    // Arrow endpoint at (280, 200), midpoint at (240, 200)

    // Card positioned to the right, overlapping arrow endpoint
    let score = score_candidate_position(
        220.0, // card_left
        150.0, // card_top
        150.0, // card_width (covers 220-370 horizontally)
        100.0, // card_height (covers 150-250 vertically)
        &screen,
        Some(vel_dir),
        object_pos,
        arrow_length,
        20.0, // right side
    );

    // Arrow endpoint (280, 200) is inside card (220-370, 150-250) -> -80 penalty
    // Arrow midpoint (240, 200) is inside card -> -40 penalty
    // Card is on same side as velocity (dot product > 0) -> no bonus
    // Right side bonus: +5
    // Score: 100 - 80 - 40 + 5 = -15
    assert!((score - (-15.0)).abs() < 0.01);
}

#[test]
fn test_score_position_opposite_to_velocity() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let object_pos = Vec2::new(400.0, 200.0);
    let vel_dir = Vec2::new(1.0, 0.0); // velocity pointing right

    // Card positioned to the left of object (opposite side from velocity)
    let score = score_candidate_position(
        200.0, // card_left
        150.0, // card_top
        150.0, // card_width
        100.0, // card_height
        &screen,
        Some(vel_dir),
        object_pos,
        80.0,
        -200.0, // left side offset (negative)
    );

    // Card center at (275, 200), to_card = (275-400, 0) = (-125, 0)
    // dot = -125 * 1.0 + 0 * 0 = -125 < 0 -> +20 bonus
    // No right side bonus (offset negative)
    // Score: 100 + 20 = 120
    assert!((score - 120.0).abs() < 0.01);
}

#[test]
fn test_clamp_card_to_screen_no_clamping_needed() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let (x, y) = clamp_card_to_screen(200.0, 200.0, 150.0, 100.0, &screen);
    assert_eq!(x, 200.0);
    assert_eq!(y, 200.0);
}

#[test]
fn test_clamp_card_to_screen_clamp_left() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let (x, y) = clamp_card_to_screen(-50.0, 200.0, 150.0, 100.0, &screen);
    assert_eq!(x, 5.0); // left + 5
    assert_eq!(y, 200.0);
}

#[test]
fn test_clamp_card_to_screen_clamp_right() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let (x, y) = clamp_card_to_screen(700.0, 200.0, 150.0, 100.0, &screen);
    // right - card_width - 5 = 800 - 150 - 5 = 645
    assert_eq!(x, 645.0);
    assert_eq!(y, 200.0);
}

#[test]
fn test_clamp_card_to_screen_clamp_bottom() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let (x, y) = clamp_card_to_screen(200.0, 500.0, 150.0, 100.0, &screen);
    assert_eq!(x, 200.0);
    // usable_bottom - card_height = 550 - 100 = 450
    assert_eq!(y, 450.0);
}

#[test]
fn test_clamp_card_to_screen_clamp_top() {
    let screen = ScreenBounds {
        left: 0.0,
        right: 800.0,
        top: 0.0,
        usable_bottom: 550.0,
    };

    let (x, y) = clamp_card_to_screen(200.0, -20.0, 150.0, 100.0, &screen);
    assert_eq!(x, 200.0);
    assert_eq!(y, 5.0); // top + 5
}
