//! UI module providing egui-based interface panels.

mod collision_notification;
mod info_panel;
mod time_controls;

use bevy::prelude::*;

pub use collision_notification::*;
pub use info_panel::*;
pub use time_controls::*;

/// Plugin that adds all UI systems.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>().add_systems(
            Update,
            (time_controls_panel, info_panel, collision_notification).chain(),
        );
    }
}

/// Global UI state.
#[derive(Resource)]
pub struct UiState {
    /// Whether the info panel is expanded.
    pub info_panel_open: bool,
    /// Display units for position/velocity.
    pub display_units: DisplayUnits,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            info_panel_open: true,
            display_units: DisplayUnits::Km,
        }
    }
}

/// Units for displaying position and velocity.
#[derive(Default, Clone, Copy, PartialEq)]
pub enum DisplayUnits {
    /// Kilometers / km/s
    #[default]
    Km,
    /// Astronomical Units / AU/day
    Au,
}
