//! Keys and Emacs `kbd`-style key descriptions such as `"hello C-x C-s"`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Char(char),
    Return,
    Tab,
    Backspace,
    Escape,
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub code: KeyCode,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
    /// Cmd on macOS, written `s-`.
    pub super_: bool,
}

impl Key {
    pub fn plain(code: KeyCode) -> Self {
        Key {
            code,
            ctrl: false,
            meta: false,
            shift: false,
            super_: false,
        }
    }
}

/// Writes the key the way `parse_keys` reads it, such as `C-x`, `M-S-a`, `s-z` or `<left>`.
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for (held, prefix) in [
            (self.ctrl, "C-"),
            (self.meta, "M-"),
            (self.shift, "S-"),
            (self.super_, "s-"),
        ] {
            if held {
                f.write_str(prefix)?;
            }
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("SPC"),
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Return => f.write_str("RET"),
            KeyCode::Tab => f.write_str("TAB"),
            KeyCode::Backspace => f.write_str("DEL"),
            KeyCode::Escape => f.write_str("ESC"),
            KeyCode::Left => f.write_str("<left>"),
            KeyCode::Right => f.write_str("<right>"),
            KeyCode::Up => f.write_str("<up>"),
            KeyCode::Down => f.write_str("<down>"),
        }
    }
}

/// Parses whitespace-separated words like Emacs's `kbd`: `C-x`, `M-S-a`, `s-z`, `RET`, `SPC`, `<left>`;
/// a word without modifiers that is not a key name types each of its characters.
pub fn parse_keys(s: &str) -> Result<Vec<Key>, String> {
    let mut keys = Vec::new();
    for word in s.split_whitespace() {
        let mut key = Key::plain(KeyCode::Char(' '));
        let mut rest = word;
        while rest.len() > 2 && rest.as_bytes()[1] == b'-' {
            match rest.as_bytes()[0] {
                b'C' => key.ctrl = true,
                b'M' => key.meta = true,
                b'S' => key.shift = true,
                b's' => key.super_ = true,
                _ => break,
            }
            rest = &rest[2..];
        }
        let named = match rest {
            "RET" => Some(KeyCode::Return),
            "TAB" => Some(KeyCode::Tab),
            "DEL" => Some(KeyCode::Backspace),
            "ESC" => Some(KeyCode::Escape),
            "SPC" => Some(KeyCode::Char(' ')),
            "<left>" => Some(KeyCode::Left),
            "<right>" => Some(KeyCode::Right),
            "<up>" => Some(KeyCode::Up),
            "<down>" => Some(KeyCode::Down),
            _ => None,
        };
        let modified = key.ctrl || key.meta || key.shift || key.super_;
        let mut chars = rest.chars();
        match (named, chars.next(), chars.next()) {
            (Some(code), _, _) => keys.push(Key { code, ..key }),
            (None, Some(c), None) => keys.push(Key {
                code: KeyCode::Char(c),
                ..key
            }),
            _ if modified => return Err(format!("bad key {word:?}")),
            _ => keys.extend(rest.chars().map(|c| Key::plain(KeyCode::Char(c)))),
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctrl(c: char) -> Key {
        Key {
            ctrl: true,
            ..Key::plain(KeyCode::Char(c))
        }
    }

    #[test]
    fn parses_words_modifiers_and_names() {
        let keys = parse_keys("hé C-x C-s M-S-a RET SPC C-- <left> C-<down>").unwrap();
        assert_eq!(
            keys,
            [
                Key::plain(KeyCode::Char('h')),
                Key::plain(KeyCode::Char('é')),
                ctrl('x'),
                ctrl('s'),
                Key {
                    meta: true,
                    shift: true,
                    ..Key::plain(KeyCode::Char('a'))
                },
                Key::plain(KeyCode::Return),
                Key::plain(KeyCode::Char(' ')),
                ctrl('-'),
                Key::plain(KeyCode::Left),
                Key {
                    ctrl: true,
                    ..Key::plain(KeyCode::Down)
                },
            ]
        );
    }

    #[test]
    fn display_round_trips_through_parse() {
        let script = "h C-x M-S-a RET SPC C-- TAB DEL ESC <left> C-<down> <up> <right> C-s-a s-z";
        let keys = parse_keys(script).unwrap();
        let shown: Vec<String> = keys.iter().map(Key::to_string).collect();
        assert_eq!(shown.join(" "), script);
        let cmd = keys[keys.len() - 2];
        assert!(cmd.ctrl && cmd.super_ && !cmd.meta);
    }

    #[test]
    fn rejects_modified_words() {
        assert!(parse_keys("C-xy").is_err());
    }
}
