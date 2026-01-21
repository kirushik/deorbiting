//! Phosphor icon definitions for the UI.
//!
//! Provides icon constants using the Phosphor icon font.
//! Icons are initialized via `setup_fonts` when the app starts.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

/// Resource to track if fonts have been initialized.
#[derive(Resource, Default)]
pub struct FontsInitialized(pub bool);

/// System to initialize Phosphor icon fonts.
/// Runs in EguiPrimaryContextPass where the egui context is guaranteed to be ready.
pub fn setup_fonts(mut contexts: EguiContexts, mut initialized: ResMut<FontsInitialized>) {
    if initialized.0 {
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);

    ctx.set_fonts(fonts);
    initialized.0 = true;

    info!("Phosphor icon fonts initialized");
}

// Re-export commonly used icons with semantic names for our app.
// Browse all icons at https://phosphoricons.com/

/// Play icon (triangle pointing right)
pub const PLAY: &str = egui_phosphor::regular::PLAY;
/// Pause icon (two vertical bars)
pub const PAUSE: &str = egui_phosphor::regular::PAUSE;
/// Reset/reload icon (circular arrow)
pub const RESET: &str = egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE;
/// Help/question icon
pub const HELP: &str = egui_phosphor::regular::QUESTION;
/// Menu/hamburger icon
pub const MENU: &str = egui_phosphor::regular::LIST;
/// Close/X icon
pub const CLOSE: &str = egui_phosphor::regular::X;
/// Expand/chevron down
pub const EXPAND: &str = egui_phosphor::regular::CARET_DOWN;
/// Collapse/chevron up
pub const COLLAPSE: &str = egui_phosphor::regular::CARET_UP;
/// Plus/add icon
pub const ADD: &str = egui_phosphor::regular::PLUS;
/// Delete/trash icon
pub const DELETE: &str = egui_phosphor::regular::TRASH;

// Celestial body icons
/// Sun icon
pub const SUN: &str = egui_phosphor::regular::SUN;
/// Planet/globe icon
pub const PLANET: &str = egui_phosphor::regular::GLOBE;
/// Moon icon
pub const MOON: &str = egui_phosphor::regular::MOON;
/// Star/asteroid icon
pub const ASTEROID: &str = egui_phosphor::regular::ASTERISK;

// Deflection method icons
/// Rocket/kinetic impactor icon
pub const KINETIC: &str = egui_phosphor::regular::ROCKET;
/// Nuclear/radiation icon
pub const NUCLEAR: &str = egui_phosphor::regular::RADIOACTIVE;
/// Split/atom icon
pub const NUCLEAR_SPLIT: &str = egui_phosphor::regular::ATOM;
/// Ion beam/lightning icon
pub const ION_BEAM: &str = egui_phosphor::regular::LIGHTNING;
/// Gravity tractor/magnet icon
pub const GRAVITY_TRACTOR: &str = egui_phosphor::regular::MAGNET;
/// Laser icon
pub const LASER: &str = egui_phosphor::regular::CROSSHAIR;
/// Solar sail icon
pub const SOLAR_SAIL: &str = egui_phosphor::regular::WIND;

// Status icons
/// Warning/alert icon
pub const WARNING: &str = egui_phosphor::regular::WARNING;
/// Success/check icon
pub const SUCCESS: &str = egui_phosphor::regular::CHECK_CIRCLE;
/// Info icon
pub const INFO: &str = egui_phosphor::regular::INFO;
/// Clock/time icon
pub const CLOCK: &str = egui_phosphor::regular::CLOCK;
/// Target/crosshair icon
pub const TARGET: &str = egui_phosphor::regular::CROSSHAIR;
/// Arrow right (for escape trajectory)
pub const ARROW_RIGHT: &str = egui_phosphor::regular::ARROW_RIGHT;
/// Orbit/path icon
pub const ORBIT: &str = egui_phosphor::regular::PATH;
/// Fuel/gas icon
pub const FUEL: &str = egui_phosphor::regular::GAS_PUMP;

// Scenario icons
/// Earth collision scenario
pub const COLLISION: &str = egui_phosphor::regular::WARNING_CIRCLE;
/// Flyby scenario
pub const FLYBY: &str = egui_phosphor::regular::ARROWS_OUT_LINE_HORIZONTAL;
/// Slingshot/gravity assist
pub const SLINGSHOT: &str = egui_phosphor::regular::SHUFFLE;
/// Interstellar/stars
pub const INTERSTELLAR: &str = egui_phosphor::regular::SHOOTING_STAR;
/// Challenge/trophy
pub const CHALLENGE: &str = egui_phosphor::regular::TROPHY;
/// Sandbox/tools
pub const SANDBOX: &str = egui_phosphor::regular::WRENCH;
