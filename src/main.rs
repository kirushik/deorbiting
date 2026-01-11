use bevy::prelude::*;

mod ephemeris;
mod types;

use ephemeris::Ephemeris;
use types::SimulationTime;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    // Spawn 2D camera for now
    commands.spawn(Camera2d);

    // Initialize ephemeris resource
    commands.insert_resource(Ephemeris::default());

    // Initialize simulation time
    commands.insert_resource(SimulationTime::default());

    info!("Deorbiting simulation initialized");
}
