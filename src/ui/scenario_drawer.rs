//! Scenario drawer - a non-modal bottom drawer for scenario selection.
//!
//! Slides up from the dock, showing scenario cards. Clicking a card
//! loads that scenario immediately. The drawer doesn't block viewport
//! interaction - it's just an overlay.

use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};

use crate::scenarios::{CurrentScenario, LoadScenarioEvent, SCENARIOS};

/// Resource for scenario drawer state.
#[derive(Resource, Default)]
pub struct ScenarioDrawerState {
    /// Whether the drawer is open.
    pub open: bool,
    /// Animation progress (0.0 = closed, 1.0 = open).
    pub animation_progress: f32,
}

/// Colors for the drawer UI.
mod colors {
    use bevy_egui::egui::Color32;

    pub const DRAWER_BG: Color32 = Color32::from_rgba_premultiplied(26, 26, 36, 240);
    pub const CARD_BG: Color32 = Color32::from_rgba_premultiplied(40, 40, 55, 255);
    pub const CARD_HOVER: Color32 = Color32::from_rgba_premultiplied(50, 50, 70, 255);
    pub const CARD_SELECTED: Color32 = Color32::from_rgba_premultiplied(60, 80, 100, 255);
    pub const CARD_BORDER: Color32 = Color32::from_rgb(80, 80, 100);
}

/// System to render the scenario drawer.
pub fn scenario_drawer_system(
    mut contexts: EguiContexts,
    mut drawer_state: ResMut<ScenarioDrawerState>,
    current_scenario: Res<CurrentScenario>,
    mut load_events: EventWriter<LoadScenarioEvent>,
    time: Res<Time>,
) {
    // Animate drawer open/close (~150ms duration)
    let target = if drawer_state.open { 1.0 } else { 0.0 };
    let speed = 12.0; // Higher = faster, 12.0 ≈ 150ms to 90% completion
    let delta = target - drawer_state.animation_progress;
    drawer_state.animation_progress += delta * speed * time.delta_secs();
    drawer_state.animation_progress = drawer_state.animation_progress.clamp(0.0, 1.0);

    if drawer_state.animation_progress < 0.01 {
        return;
    }

    let Some(ctx) = contexts.try_ctx_mut() else {
        return;
    };

    let drawer_height = 200.0;
    let visible_height = drawer_height * drawer_state.animation_progress;
    let dock_height = 56.0;

    egui::Area::new(egui::Id::new("scenario_drawer"))
        .fixed_pos(egui::pos2(0.0, ctx.screen_rect().height() - dock_height - visible_height))
        .show(ctx, |ui| {
            let frame = egui::Frame::none()
                .fill(colors::DRAWER_BG)
                .inner_margin(egui::Margin::symmetric(20.0, 16.0))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(60, 60, 80)));

            frame.show(ui, |ui| {
                ui.set_min_width(ctx.screen_rect().width());
                ui.set_min_height(drawer_height);

                // Header
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Select Scenario").size(18.0).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        use crate::ui::icons;
                        if ui.add(egui::Button::new(egui::RichText::new(icons::CLOSE).size(16.0))
                            .min_size(egui::vec2(28.0, 28.0)))
                            .on_hover_text("Close (Esc)").clicked() {
                            drawer_state.open = false;
                        }
                    });
                });

                ui.add_space(12.0);

                // Scenario cards
                egui::ScrollArea::horizontal()
                    .max_height(drawer_height - 60.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 16.0;

                            for scenario in SCENARIOS.iter() {
                                let is_current = scenario.id == current_scenario.id;

                                if render_scenario_card(ui, scenario.id, scenario.name, scenario.description, is_current) {
                                    load_events.send(LoadScenarioEvent {
                                        scenario_id: scenario.id,
                                    });
                                    drawer_state.open = false;
                                }
                            }
                        });
                    });
            });
        });

    // Close on click outside
    if drawer_state.open && ctx.input(|i| i.pointer.any_pressed()) {
        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            let drawer_top = ctx.screen_rect().height() - dock_height - visible_height;
            if pos.y < drawer_top {
                drawer_state.open = false;
            }
        }
    }
}

/// System to handle keyboard shortcuts for the drawer.
pub fn scenario_drawer_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut drawer_state: ResMut<ScenarioDrawerState>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        drawer_state.open = !drawer_state.open;
    }
}

/// Render a single scenario card. Returns true if clicked.
fn render_scenario_card(
    ui: &mut egui::Ui,
    id: &str,
    name: &str,
    description: &str,
    is_current: bool,
) -> bool {
    let card_width = 170.0;
    let card_height = 110.0;

    let (icon, icon_color) = scenario_icon(id);

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(card_width, card_height),
        egui::Sense::click(),
    );

    let bg_color = if is_current {
        colors::CARD_SELECTED
    } else if response.hovered() {
        colors::CARD_HOVER
    } else {
        colors::CARD_BG
    };

    // Draw card background
    ui.painter().rect(
        rect,
        6.0,
        bg_color,
        egui::Stroke::new(
            if is_current { 2.0 } else { 1.0 },
            if is_current { egui::Color32::from_rgb(85, 153, 221) } else { colors::CARD_BORDER },
        ),
    );

    let inner_rect = rect.shrink(10.0);

    // Icon (Phosphor icon)
    ui.painter().text(
        egui::pos2(inner_rect.center().x, inner_rect.top() + 22.0),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(28.0),
        icon_color,
    );

    // Name
    ui.painter().text(
        egui::pos2(inner_rect.center().x, inner_rect.top() + 55.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(15.0),
        egui::Color32::WHITE,
    );

    // Description (truncated)
    let desc_short = if description.len() > 35 {
        format!("{}...", &description[..32])
    } else {
        description.to_string()
    };
    ui.painter().text(
        egui::pos2(inner_rect.center().x, inner_rect.top() + 80.0),
        egui::Align2::CENTER_CENTER,
        desc_short,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(160, 160, 170),
    );

    // Current indicator
    if is_current {
        ui.painter().text(
            egui::pos2(inner_rect.right() - 2.0, inner_rect.top() + 2.0),
            egui::Align2::RIGHT_TOP,
            "*",
            egui::FontId::proportional(14.0),
            egui::Color32::from_rgb(85, 221, 136),
        );
    }

    response.clicked()
}

/// Get icon for a scenario based on its ID.
/// Returns (icon_text, color)
fn scenario_icon(id: &str) -> (&'static str, egui::Color32) {
    use crate::ui::icons;
    match id {
        "earth_collision" => (icons::COLLISION, egui::Color32::from_rgb(255, 100, 100)),
        "apophis_flyby" => (icons::FLYBY, egui::Color32::from_rgb(200, 200, 100)),
        "jupiter_slingshot" => (icons::SLINGSHOT, egui::Color32::from_rgb(255, 180, 100)),
        "interstellar_visitor" => (icons::INTERSTELLAR, egui::Color32::from_rgb(150, 150, 255)),
        "deflection_challenge" => (icons::CHALLENGE, egui::Color32::from_rgb(255, 200, 80)),
        "sandbox" => (icons::SANDBOX, egui::Color32::from_rgb(100, 200, 255)),
        _ => (icons::HELP, egui::Color32::WHITE),
    }
}
