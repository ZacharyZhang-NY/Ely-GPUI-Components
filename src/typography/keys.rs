use gpui::Keystroke;

use crate::theme::Platform;

impl From<Platform> for gpui::OperatingSystem {
    fn from(platform: Platform) -> Self {
        match platform {
            Platform::Mac => Self::Mac,
            Platform::Windows => Self::Windows,
            Platform::Linux => Self::Linux,
        }
    }
}

/// One keystroke in gpui key syntax, `secondary` read for `platform`: Command on the Mac, Control elsewhere.
pub fn keystroke(source: &str, platform: Platform) -> Keystroke {
    Keystroke::parse_with_os(source, platform.into())
        .unwrap_or_else(|error| panic!("bad keystroke {source:?}: {error}"))
}

/// Caps for one keystroke, modifiers first, in platform order.
pub fn keystroke_labels(stroke: &Keystroke, platform: Platform) -> Vec<String> {
    let m = &stroke.modifiers;
    let mac = platform == Platform::Mac;
    let super_key = if platform == Platform::Windows {
        "Win"
    } else {
        "Super"
    };
    let order: [(bool, &str, &str); 5] = [
        (m.control, "⌃", "Ctrl"),
        (m.alt, "⌥", "Alt"),
        (m.shift, "⇧", "Shift"),
        (m.platform, "⌘", super_key),
        (m.function, "fn", "Fn"),
    ];
    let mut labels: Vec<String> = order
        .iter()
        .filter(|(held, _, _)| *held)
        .map(|(_, mac_label, label)| if mac { *mac_label } else { *label }.to_string())
        .collect();
    labels.push(key_label(&stroke.key, platform));
    labels
}

/// Cap text for a gpui key name.
pub fn key_label(key: &str, platform: Platform) -> String {
    let mac = platform == Platform::Mac;
    let named = match key {
        "enter" => Some(if mac { "↩" } else { "Enter" }),
        "escape" => Some(if mac { "⎋" } else { "Esc" }),
        "tab" => Some(if mac { "⇥" } else { "Tab" }),
        "backspace" => Some(if mac { "⌫" } else { "Backspace" }),
        "delete" => Some(if mac { "⌦" } else { "Del" }),
        "space" => Some("Space"),
        "up" => Some("↑"),
        "down" => Some("↓"),
        "left" => Some("←"),
        "right" => Some("→"),
        "home" => Some("Home"),
        "end" => Some("End"),
        "pageup" => Some("PgUp"),
        "pagedown" => Some("PgDn"),
        _ => None,
    };
    match named {
        Some(label) => label.to_string(),
        None => key.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &'static str) -> Keystroke {
        Keystroke::parse(source).expect("valid keystroke")
    }

    #[test]
    fn mac_uses_symbols_in_apple_order() {
        let labels = keystroke_labels(&parse("cmd-shift-p"), Platform::Mac);
        assert_eq!(labels, ["⇧", "⌘", "P"]);
        let labels = keystroke_labels(&parse("ctrl-alt-enter"), Platform::Mac);
        assert_eq!(labels, ["⌃", "⌥", "↩"]);
    }

    #[test]
    fn other_platforms_spell_modifiers() {
        let labels = keystroke_labels(&parse("ctrl-shift-p"), Platform::Windows);
        assert_eq!(labels, ["Ctrl", "Shift", "P"]);
        let labels = keystroke_labels(&parse("cmd-escape"), Platform::Linux);
        assert_eq!(labels, ["Super", "Esc"]);
    }

    #[test]
    fn secondary_reads_for_the_platform() {
        let mac = keystroke_labels(&keystroke("secondary-s", Platform::Mac), Platform::Mac);
        assert_eq!(mac, ["⌘", "S"]);
        let windows = Platform::Windows;
        assert_eq!(
            keystroke_labels(&keystroke("secondary-s", windows), windows),
            ["Ctrl", "S"]
        );
    }

    #[test]
    fn keys_keep_their_names_or_upper_case() {
        assert_eq!(key_label("f12", Platform::Mac), "F12");
        assert_eq!(key_label("space", Platform::Windows), "Space");
        assert_eq!(key_label("left", Platform::Linux), "←");
    }

    #[test]
    fn a_secondary_keystroke_follows_the_platform_asked_for() {
        let mac = keystroke("secondary-s", Platform::Mac);
        assert!(
            mac.modifiers.platform && !mac.modifiers.control,
            "⌘S on the Mac"
        );
        for platform in [Platform::Windows, Platform::Linux] {
            let pc = keystroke("secondary-s", platform);
            assert!(
                pc.modifiers.control && !pc.modifiers.platform,
                "Ctrl+S on {platform:?}"
            );
        }
    }
}
