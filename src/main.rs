//! Deorbiting - Orbital Mechanics Simulator
//!
//! A desktop application for simulating asteroid deorbiting missions
//! with accurate orbital mechanics and a user-friendly interface.

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use deorbiting::asteroid::{handle_reset, AsteroidCounter, ResetEvent};
use deorbiting::camera::CameraPlugin;
use deorbiting::collision::CollisionPlugin;
use deorbiting::continuous::ContinuousPlugin;
use deorbiting::ephemeris::Ephemeris;
use deorbiting::input::InputPlugin;
use deorbiting::interceptor::InterceptorPlugin;
use deorbiting::physics::PhysicsPlugin;
use deorbiting::prediction::PredictionPlugin;
use deorbiting::render::RenderPlugin;
use deorbiting::scenarios::ScenarioPlugin;
use deorbiting::time::TimePlugin;
use deorbiting::types::SimulationTime;
use deorbiting::ui::velocity_handle::VelocityHandlePlugin;
use deorbiting::ui::UiPlugin;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin)
        // Diagnostic plugins for performance monitoring
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
        .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
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
            PredictionPlugin,
            VelocityHandlePlugin,
            ScenarioPlugin,
            InterceptorPlugin,
            ContinuousPlugin,
        ))
        // Handle reset events
        .add_systems(Update, handle_reset);

    // Log diagnostics to console in debug builds
    #[cfg(debug_assertions)]
    app.add_plugins(bevy::diagnostic::LogDiagnosticsPlugin::default());

    app.run();
}
