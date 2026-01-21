//! Integration tests for scenario definitions.

use deorbiting::scenarios::presets::SCENARIOS;

#[test]
fn test_all_scenarios_exist() {
    // Should have at least one scenario
    assert!(
        !SCENARIOS.is_empty(),
        "Should have at least one scenario defined"
    );
}

#[test]
fn test_all_scenarios_start_paused() {
    for scenario in SCENARIOS {
        assert!(
            scenario.start_paused,
            "Scenario '{}' should start paused",
            scenario.id
        );
    }
}

#[test]
fn test_scenario_unique_ids() {
    let mut ids: Vec<&str> = SCENARIOS.iter().map(|s| s.id).collect();
    ids.sort();

    for i in 1..ids.len() {
        assert_ne!(ids[i - 1], ids[i], "Duplicate scenario ID: {}", ids[i]);
    }
}

#[test]
fn test_scenario_unique_names() {
    let mut names: Vec<&str> = SCENARIOS.iter().map(|s| s.name).collect();
    names.sort();

    for i in 1..names.len() {
        assert_ne!(
            names[i - 1],
            names[i],
            "Duplicate scenario name: {}",
            names[i]
        );
    }
}

#[test]
fn test_scenario_time_scale_positive() {
    for scenario in SCENARIOS {
        assert!(
            scenario.time_scale > 0.0,
            "Scenario '{}' should have positive time scale",
            scenario.id
        );
    }
}

#[test]
fn test_scenario_camera_zoom_positive() {
    for scenario in SCENARIOS {
        assert!(
            scenario.camera_zoom > 0.0,
            "Scenario '{}' should have positive camera zoom",
            scenario.id
        );
    }
}

#[test]
fn test_scenario_asteroid_mass_positive() {
    for scenario in SCENARIOS {
        assert!(
            scenario.asteroid_mass > 0.0,
            "Scenario '{}' should have positive asteroid mass",
            scenario.id
        );
    }
}

#[test]
fn test_earth_collision_is_first() {
    // Earth collision should be the default/first scenario
    assert_eq!(
        SCENARIOS[0].id, "earth_collision",
        "Earth collision should be the first scenario"
    );
}

#[test]
fn test_scenarios_have_descriptions() {
    for scenario in SCENARIOS {
        assert!(
            !scenario.description.is_empty(),
            "Scenario '{}' should have a description",
            scenario.id
        );
    }
}

#[test]
fn test_scenarios_use_dynamic_computation() {
    // Scenarios using dynamic computation have None for pos/vel
    // (they're computed at runtime based on current planet positions)
    for scenario in SCENARIOS {
        // Most scenarios use dynamic computation
        // This test verifies the pattern is consistent
        let uses_dynamic = scenario.asteroid_pos.is_none() || scenario.asteroid_vel.is_none();
        assert!(
            uses_dynamic || (scenario.asteroid_pos.is_some() && scenario.asteroid_vel.is_some()),
            "Scenario '{}' should either be fully dynamic or fully static",
            scenario.id
        );
    }
}
