//! Deorbiting - Orbital Mechanics Simulator
//!
//! A desktop application for simulating asteroid deorbiting missions
//! with accurate orbital mechanics and a user-friendly interface.

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

mod camera;
mod ephemeris;
mod input;
mod render;
mod time;
mod types;

use camera::CameraPlugin;
use ephemeris::Ephemeris;
use input::InputPlugin;
use render::RenderPlugin;
use time::TimePlugin;
use types::SimulationTime;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // Insert resources before plugins that depend on them
        .insert_resource(Ephemeris::default())
        .insert_resource(SimulationTime::default())
        // Add simulation plugins
        .add_plugins((CameraPlugin, TimePlugin, RenderPlugin, InputPlugin))
        .run();
}
