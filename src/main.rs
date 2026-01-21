//! Deorbiting - Orbital Mechanics Simulator
//!
//! A desktop application for simulating asteroid deorbiting missions
//! with accurate orbital mechanics and a user-friendly interface.

use bevy::prelude::*;
use bevy_egui::EguiPlugin;

use deorbiting::asteroid::{AsteroidCounter, ResetEvent, handle_reset};
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
use deorbiting::ui::UiPlugin;
use deorbiting::ui::velocity_handle::VelocityHandlePlugin;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        // Diagnostic plugins for performance monitoring
        .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
        .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin::default())
        // Insert resources before plugins that depend on them
        .insert_resource(Ephemeris::default())
        .insert_resource(SimulationTime::default())
        .insert_resource(AsteroidCounter::default())
        // Register message channels
        .init_resource::<Messages<ResetEvent>>()
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
