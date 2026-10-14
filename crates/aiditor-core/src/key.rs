//! Keys and Emacs `kbd`-style key descriptions such as `"hello C-x C-s"`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyCode {
    Char(char),
    Return,
    Tab,
    Backspace,
    Escape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub code: KeyCode,
    pub ctrl: bool,
    pub meta: bool,
    pub shift: bool,
}

impl Key {
    pub fn plain(code: KeyCode) -> Self {
        Key {
            code,
            ctrl: false,
            meta: false,
            shift: false,
        }
    }
}

/// Parses whitespace-separated words like Emacs's `kbd`: `C-x`, `M-S-a`, `RET`, `SPC`;
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
            _ => None,
        };
        let modified = key.ctrl || key.meta || key.shift;
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
        let keys = parse_keys("hé C-x C-s M-S-a RET SPC C--").unwrap();
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
            ]
        );
    }

    #[test]
    fn rejects_modified_words() {
        assert!(parse_keys("C-xy").is_err());
    }
}
