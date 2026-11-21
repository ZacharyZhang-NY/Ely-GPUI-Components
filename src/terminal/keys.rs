use gpui::Keystroke;

/// The bytes a key sends to the shell as xterm does, or none when the app keeps it or text input types it.
pub(crate) fn bytes(keystroke: &Keystroke, app_cursor: bool) -> Option<Vec<u8>> {
    let held = &keystroke.modifiers;
    if held.platform {
        return None;
    }
    let code = 1 + u8::from(held.shift) + 2 * u8::from(held.alt) + 4 * u8::from(held.control);
    let cursor = |end: char| match (code, app_cursor) {
        (1, true) => format!("\x1bO{end}"),
        (1, false) => format!("\x1b[{end}"),
        _ => format!("\x1b[1;{code}{end}"),
    };
    let tilde = |number: u8| match code {
        1 => format!("\x1b[{number}~"),
        _ => format!("\x1b[{number};{code}~"),
    };
    let function = |end: char| match code {
        1 => format!("\x1bO{end}"),
        _ => format!("\x1b[1;{code}{end}"),
    };
    let sent = match keystroke.key.as_str() {
        "enter" if held.alt => "\x1b\r".to_string(),
        "enter" => "\r".to_string(),
        "backspace" if held.control => "\x08".to_string(),
        "backspace" if held.alt => "\x1b\x7f".to_string(),
        "backspace" => "\x7f".to_string(),
        "tab" if held.shift => "\x1b[Z".to_string(),
        "tab" => "\t".to_string(),
        "escape" => "\x1b".to_string(),
        "up" => cursor('A'),
        "down" => cursor('B'),
        "right" => cursor('C'),
        "left" => cursor('D'),
        "home" => cursor('H'),
        "end" => cursor('F'),
        "insert" => tilde(2),
        "delete" => tilde(3),
        "pageup" => tilde(5),
        "pagedown" => tilde(6),
        "f1" => function('P'),
        "f2" => function('Q'),
        "f3" => function('R'),
        "f4" => function('S'),
        "f5" => tilde(15),
        "f6" => tilde(17),
        "f7" => tilde(18),
        "f8" => tilde(19),
        "f9" => tilde(20),
        "f10" => tilde(21),
        "f11" => tilde(23),
        "f12" => tilde(24),
        key if held.control => return control(key, held.alt),
        _ => return None,
    };
    Some(sent.into_bytes())
}

/// Ctrl with a letter or symbol: its control code, after Escape when Alt is held too.
fn control(key: &str, alt: bool) -> Option<Vec<u8>> {
    let code = match key {
        "space" | "@" | "2" => 0,
        "[" | "3" => 0x1b,
        "\\" | "4" => 0x1c,
        "]" | "5" => 0x1d,
        "^" | "6" => 0x1e,
        "_" | "-" | "7" => 0x1f,
        "?" | "8" => 0x7f,
        letter if letter.len() == 1 && letter.as_bytes()[0].is_ascii_lowercase() => {
            letter.as_bytes()[0] - b'a' + 1
        }
        _ => return None,
    };
    Some(if alt { vec![0x1b, code] } else { vec![code] })
}

/// Pasted text as the shell takes it: bracketed when it asked, line ends as carriage returns.
pub(crate) fn paste(text: &str, bracketed: bool) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if bracketed {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sent(keys: &'static str, app_cursor: bool) -> Option<Vec<u8>> {
        bytes(&Keystroke::parse(keys).expect("a keystroke"), app_cursor)
    }

    #[test]
    fn keys_send_what_xterm_sends() {
        assert_eq!(sent("enter", false), Some(b"\r".to_vec()));
        assert_eq!(sent("up", false), Some(b"\x1b[A".to_vec()));
        assert_eq!(
            sent("up", true),
            Some(b"\x1bOA".to_vec()),
            "application cursor keys"
        );
        assert_eq!(sent("shift-up", true), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(sent("ctrl-c", false), Some(vec![3]));
        assert_eq!(sent("ctrl-alt-a", false), Some(vec![0x1b, 1]));
        assert_eq!(sent("ctrl-[", false), Some(vec![0x1b]));
        assert_eq!(sent("f5", false), Some(b"\x1b[15~".to_vec()));
        assert_eq!(sent("ctrl-delete", false), Some(b"\x1b[3;5~".to_vec()));
        assert_eq!(sent("shift-tab", false), Some(b"\x1b[Z".to_vec()));
    }

    #[test]
    fn app_shortcuts_and_typed_text_stay_out() {
        assert_eq!(sent("cmd-c", false), None);
        assert_eq!(sent("a", false), None, "text input types letters");
        assert_eq!(sent("ctrl-1", false), None);
    }

    #[test]
    fn pastes_bracket_when_asked() {
        assert_eq!(paste("a\nb", false), b"a\rb".to_vec());
        assert_eq!(
            paste("a\x1b[201~b", true),
            b"\x1b[200~a[201~b\x1b[201~".to_vec()
        );
    }
}
