//! UI module providing egui-based interface.
//!
//! Redesigned for modeless, direct-manipulation interaction following
//! Raskin's principles and Apple HIG.

mod banners;
mod box_selection;
mod context_card;
mod dock;
pub mod icons;
mod radial_menu;
mod scenario_drawer;
pub mod velocity_handle;

// Keep asteroid_placement for click-to-spawn functionality
mod asteroid_placement;

use bevy::prelude::*;

pub use banners::BannerState;
// context_card exports are used internally
pub use dock::{AsteroidListState, HelpTooltipState};
pub use radial_menu::RadialMenuState;
pub use scenario_drawer::ScenarioDrawerState;

// Re-export asteroid placement for modeless spawning
pub use asteroid_placement::{handle_asteroid_placement, update_placement_cursor};

/// Plugin that adds all UI systems.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app
            // Initialize resources
            .init_resource::<UiState>()
            .init_resource::<ScenarioDrawerState>()
            .init_resource::<HelpTooltipState>()
            .init_resource::<RadialMenuState>()
            .init_resource::<BannerState>()
            .init_resource::<AsteroidPlacementMode>()
            .init_resource::<ActiveNotification>()
            .init_resource::<icons::FontsInitialized>()
            .init_resource::<box_selection::BoxSelectionState>()
            .init_resource::<AsteroidListState>()
            // Add systems
            .add_systems(
                Update,
                (
                    // Font initialization (runs once)
                    icons::setup_fonts,
                    // Dock (bottom bar with all controls)
                    dock::dock_system,
                    // Scenario drawer (slides up from dock)
                    scenario_drawer::scenario_drawer_system,
                    scenario_drawer::scenario_drawer_keyboard,
                    // Context card (floating info near selection)
                    context_card::context_card_system,
                    // Radial deflection menu
                    radial_menu::radial_menu_system,
                    // Outcome banners
                    banners::update_banner_state,
                    banners::animate_banners,
                    banners::banner_system,
                    // Asteroid placement (click-to-spawn)
                    handle_asteroid_placement,
                    update_placement_cursor,
                    // Box selection (drag to select)
                    box_selection::box_selection_input,
                    box_selection::render_box_selection,
                ),
            );
    }
}

/// Global UI state.
#[derive(Resource)]
pub struct UiState {
    /// Display units for position/velocity.
    pub display_units: DisplayUnits,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
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

/// Resource tracking the currently displayed collision notification.
///
/// Kept for compatibility with collision system.
#[derive(Resource, Default)]
pub struct ActiveNotification {
    /// The collision event currently being displayed, if any.
    pub current: Option<crate::collision::CollisionEvent>,
}
