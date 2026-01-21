//! Phosphor icon definitions for the UI.
//!
//! Provides icon constants using the Phosphor icon font.
//! Icons are initialized via `setup_fonts` when the app starts.
//!
//! **Important**: Always use [`icon()`] to render icons, not raw `RichText::new(ICON)`.
//! This ensures the Phosphor font is used explicitly, avoiding fallback issues.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

/// The font family name for Phosphor icons.
/// Used internally to ensure icons render with the correct font.
const PHOSPHOR_FONT: &str = "phosphor";

/// Creates a RichText for an icon using the Phosphor font explicitly.
///
/// This uses a named font family to guarantee Phosphor is used for icons,
/// avoiding any font fallback issues.
///
/// # Example
/// ```ignore
/// ui.label(icons::icon(icons::PLANET, 16.0));
/// ```
pub fn icon(icon_str: &str, size: f32) -> egui::RichText {
    egui::RichText::new(icon_str)
        .size(size)
        .family(egui::FontFamily::Name(PHOSPHOR_FONT.into()))
}

/// Creates a colored icon RichText.
pub fn icon_colored(icon_str: &str, size: f32, color: egui::Color32) -> egui::RichText {
    icon(icon_str, size).color(color)
}

/// Resource to track font initialization state.
/// UI systems should only run when this reaches 2+ (one frame after fonts are set).
#[derive(Resource, Default)]
pub struct FontsInitialized(pub u32);

/// System to initialize fonts: Inter for UI text, Phosphor for icons.
/// Runs in EguiPrimaryContextPass where the egui context is guaranteed to be ready.
///
/// Uses Inter Light (weight 300) to compensate for halation - light text on dark
/// backgrounds appears heavier than intended. This is a standard dark mode optimization.
///
/// Font family is set to exactly [Inter, Phosphor] with NO system defaults.
/// - Inter renders all regular text (A-Z, numbers, punctuation)
/// - Phosphor renders icon characters (PUA codepoints that Inter lacks)
/// - System defaults are excluded because they have fallback PUA glyphs
pub fn setup_fonts(mut contexts: EguiContexts, mut initialized: ResMut<FontsInitialized>) {
    // State machine: 0 = not started, 1 = fonts set (wait one frame), 2+ = ready
    if initialized.0 >= 2 {
        return;
    }

    // If fonts were set last frame, increment to "ready" state
    if initialized.0 == 1 {
        initialized.0 = 2;
        return;
    }

    let Ok(ctx) = contexts.ctx_mut() else {
        return;
    };

    let mut fonts = egui::FontDefinitions::default();

    // Add Inter Light for text (weight 300 for dark mode halation compensation)
    fonts.font_data.insert(
        "inter".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../assets/fonts/Inter-Light.ttf"
        ))),
    );

    // Add Phosphor icons font data
    fonts.font_data.insert(
        PHOSPHOR_FONT.to_owned(),
        egui_phosphor::Variant::Regular.font_data().into(),
    );

    // Register Phosphor as a NAMED font family for explicit icon rendering.
    // This allows icons::icon() to request this font directly, bypassing fallback.
    fonts.families.insert(
        egui::FontFamily::Name(PHOSPHOR_FONT.into()),
        vec![PHOSPHOR_FONT.to_owned()],
    );

    // Set up Proportional family with Inter as primary text font.
    // Keep system defaults for any characters Inter might lack.
    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        proportional.insert(0, "inter".to_owned());
    }

    ctx.set_fonts(fonts);
    initialized.0 = 1; // Set to 1, will become 2 next frame

    info!("Fonts initialized: Inter Light + Phosphor icons");
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
/// Success/check icon (circled)
pub const SUCCESS: &str = egui_phosphor::regular::CHECK_CIRCLE;
/// Check mark (simple)
pub const CHECK: &str = egui_phosphor::regular::CHECK;
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
