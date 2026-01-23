//! UI module providing egui-based interface.
//!
//! Redesigned for modeless, direct-manipulation interaction following
//! Raskin's principles and Apple HIG.

mod banners;
mod box_selection;
mod context_card;
pub mod deflection_helpers;
mod dock;
pub mod icons;
mod radial_menu;
mod scenario_drawer;
pub mod velocity_handle;

// Keep asteroid_placement for click-to-spawn functionality
mod asteroid_placement;

use bevy::prelude::*;
use bevy_egui::EguiPrimaryContextPass;

pub use banners::BannerState;
// context_card exports are used internally
pub use dock::HelpTooltipState;
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
            // Keyboard shortcuts don't need egui context - can stay in Update
            .add_systems(Update, scenario_drawer::scenario_drawer_keyboard)
            // Font initialization MUST run before any UI systems that use icons
            .add_systems(EguiPrimaryContextPass, icons::setup_fonts)
            // UI systems run in EguiPrimaryContextPass AFTER fonts are initialized
            // Run condition ensures fonts are ready before any icon rendering
            .add_systems(
                EguiPrimaryContextPass,
                (
                    // Dock (bottom bar with all controls)
                    dock::dock_system,
                    // Scenario drawer (slides up from dock)
                    scenario_drawer::scenario_drawer_system,
                    // Context card (floating info near selection)
                    context_card::context_card_system,
                    // Radial deflection menu
                    radial_menu::radial_menu_system,
                    // Outcome banners
                    banners::update_banner_state,
                    banners::animate_banners,
                    banners::banner_system,
                )
                    .after(icons::setup_fonts)
                    .run_if(|init: Res<icons::FontsInitialized>| init.0 >= 2),
            )
            .add_systems(
                EguiPrimaryContextPass,
                (
                    // Asteroid placement (click-to-spawn)
                    handle_asteroid_placement,
                    update_placement_cursor,
                    // Box selection (drag to select)
                    box_selection::box_selection_input,
                    box_selection::render_box_selection,
                )
                    .after(icons::setup_fonts)
                    .run_if(|init: Res<icons::FontsInitialized>| init.0 >= 2),
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
