//! Deorbiting - Orbital Mechanics Simulator
//!
//! A desktop application for simulating asteroid deorbiting missions
//! with accurate orbital mechanics and a user-friendly interface.

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

mod asteroid;
mod camera;
mod collision;
mod distortion;
mod ephemeris;
mod input;
mod physics;
mod render;
mod time;
mod types;
mod ui;

use asteroid::{handle_reset, spawn_initial_asteroid, AsteroidCounter, ResetEvent};
use camera::CameraPlugin;
use collision::CollisionPlugin;
use ephemeris::Ephemeris;
use input::InputPlugin;
use physics::PhysicsPlugin;
use render::RenderPlugin;
use time::TimePlugin;
use types::SimulationTime;
use ui::UiPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // Insert resources before plugins that depend on them
        .insert_resource(Ephemeris::default())
        .insert_resource(SimulationTime::default())
        .insert_resource(AsteroidCounter::default())
        // Register events
        .add_event::<ResetEvent>()
        // Add simulation plugins
        .add_plugins((
            CameraPlugin,
            TimePlugin,
            RenderPlugin,
            InputPlugin,
            UiPlugin,
            PhysicsPlugin,
            CollisionPlugin,
        ))
        // Spawn initial asteroid for testing
        .add_systems(Startup, spawn_initial_asteroid)
        // Handle reset events
        .add_systems(Update, handle_reset)
        .run();
}
