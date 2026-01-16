//! UI module providing egui-based interface panels.

mod asteroid_placement;
mod collision_notification;
mod info_panel;
mod interceptor_launch;
mod outcome_overlay;
mod scenario_menu;
mod time_controls;
pub mod velocity_handle;

use bevy::prelude::*;

pub use asteroid_placement::*;
pub use collision_notification::*;
pub use info_panel::*;
pub use interceptor_launch::*;
pub use outcome_overlay::*;
pub use scenario_menu::*;
pub use time_controls::*;

/// Plugin that adds all UI systems.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>()
            .init_resource::<ActiveNotification>()
            .init_resource::<AsteroidPlacementMode>()
            .init_resource::<OutcomeOverlayState>()
            .init_resource::<InterceptorLaunchState>()
            .add_event::<TogglePlacementModeEvent>()
            .add_systems(
                Update,
                (
                    time_controls_panel,
                    info_panel,
                    collision_notification,
                    handle_toggle_placement_event,
                    handle_asteroid_placement,
                    update_placement_cursor,
                    // New Phase 5 UI systems
                    scenario_menu_system,
                    scenario_menu_keyboard,
                    update_outcome_state,
                    animate_flash,
                    outcome_overlay_system,
                    interceptor_launch_system,
                ),
            );
    }
}

/// System to handle toggle placement mode events.
fn handle_toggle_placement_event(
    mut events: EventReader<TogglePlacementModeEvent>,
    mut placement_mode: ResMut<AsteroidPlacementMode>,
) {
    for _ in events.read() {
        placement_mode.active = !placement_mode.active;
        if placement_mode.active {
            info!("Asteroid placement mode activated - click to place asteroid");
        } else {
            info!("Asteroid placement mode deactivated");
        }
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

/// Resource tracking asteroid placement mode.
///
/// When active, the next click on the viewport will spawn an asteroid
/// at that location with a velocity calculated to intercept Earth.
#[derive(Resource, Default)]
pub struct AsteroidPlacementMode {
    /// Whether placement mode is active.
    pub active: bool,
}

/// Event to toggle asteroid placement mode.
#[derive(Event)]
pub struct TogglePlacementModeEvent;
