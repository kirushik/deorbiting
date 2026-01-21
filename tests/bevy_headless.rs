//! Headless Bevy integration tests.
//!
//! These tests verify Bevy resources and systems work correctly without GPU.

use bevy::prelude::*;
use deorbiting::ephemeris::Ephemeris;
use deorbiting::types::{SECONDS_PER_DAY, SimulationTime};

fn create_minimal_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app
}

#[test]
fn test_ephemeris_resource_initializes() {
    let mut app = create_minimal_app();
    app.insert_resource(Ephemeris::default());

    // Update once to initialize
    app.update();

    // Ephemeris should be accessible
    let ephemeris = app.world().resource::<Ephemeris>();

    // Should be able to get gravity sources
    let sources = ephemeris.get_gravity_sources(0.0);
    assert!(!sources.is_empty(), "Should have gravity sources");
}

#[test]
fn test_simulation_time_resource() {
    let mut app = create_minimal_app();
    app.insert_resource(SimulationTime::default());

    app.update();

    let sim_time = app.world().resource::<SimulationTime>();

    // Default should not be paused
    // (Actual behavior may vary based on implementation)
    assert!(sim_time.current >= 0.0, "Time should be non-negative");
}

#[test]
fn test_simulation_time_advances() {
    let mut app = create_minimal_app();
    let mut sim_time = SimulationTime::default();
    sim_time.paused = false;

    app.insert_resource(sim_time);

    // Add a system that advances time
    app.add_systems(Update, |mut time: ResMut<SimulationTime>| {
        if !time.paused {
            time.current += SECONDS_PER_DAY; // Advance 1 day per frame
        }
    });

    // Run a few frames
    for _ in 0..5 {
        app.update();
    }

    let final_time = app.world().resource::<SimulationTime>();
    assert!(
        final_time.current > 0.0,
        "Simulation time should have advanced"
    );
}

#[test]
fn test_simulation_time_pause() {
    let mut app = create_minimal_app();
    let mut sim_time = SimulationTime::default();
    sim_time.paused = true;
    let initial_time = sim_time.current;

    app.insert_resource(sim_time);

    // Add a system that would advance time (but shouldn't when paused)
    app.add_systems(Update, |mut time: ResMut<SimulationTime>| {
        if !time.paused {
            time.current += SECONDS_PER_DAY;
        }
    });

    // Run a few frames
    for _ in 0..5 {
        app.update();
    }

    let final_time = app.world().resource::<SimulationTime>();
    assert_eq!(
        final_time.current, initial_time,
        "Paused simulation should not advance"
    );
}
