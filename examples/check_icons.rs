//! Check what Unicode codepoints Phosphor icons use

fn main() {
    let icons = [
        ("GLOBE (Planet)", egui_phosphor::regular::GLOBE),
        ("ASTERISK (Asteroid)", egui_phosphor::regular::ASTERISK),
        (
            "ARROW_COUNTER_CLOCKWISE (Reset)",
            egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
        ),
        ("SUN", egui_phosphor::regular::SUN),
        ("MOON", egui_phosphor::regular::MOON),
        ("CHECK", egui_phosphor::regular::CHECK),
        ("ROCKET (Kinetic)", egui_phosphor::regular::ROCKET),
        ("PLAY", egui_phosphor::regular::PLAY),
        ("PAUSE", egui_phosphor::regular::PAUSE),
    ];

    for (name, icon) in icons {
        let codepoints: Vec<String> = icon
            .chars()
            .map(|c| format!("U+{:04X} ({})", c as u32, c))
            .collect();
        println!("{}: {}", name, codepoints.join(", "));
    }
}
