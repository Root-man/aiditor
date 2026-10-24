use std::ops::Range;

use aiditor_core::Msg;
use aiditor_core::key::{Key, KeyCode};
use gpui::{Bounds, Context, EntityInputHandler, Keystroke, Pixels, Point, UTF16Selection, Window};

use crate::editor::Editor;

impl Editor {
    /// Point and the marked text's length, in UTF-16 units of the text the input method sees:
    /// the buffer with the marked text at point.
    fn ime_point(&self) -> (usize, usize) {
        let point = self
            .core
            .current()
            .text()
            .char_to_utf16_cu(self.core.point());
        let marked = self.marked.as_deref().unwrap_or_default();
        (point, marked.encode_utf16().count())
    }

    /// A UTF-16 offset in the input method's text as one in the buffer; inside the marked text
    /// is at point.
    fn to_buffer(&self, offset: usize) -> usize {
        let (point, marked) = self.ime_point();
        match offset {
            o if o <= point => o,
            o if o >= point + marked => o - marked,
            _ => point,
        }
    }

    /// The buffer range an input method's range covers, or `None` if it covers none.
    fn buffer_range(&self, range: Option<Range<usize>>) -> Option<Range<usize>> {
        range
            .map(|r| self.to_buffer(r.start)..self.to_buffer(r.end))
            .filter(|r| !r.is_empty())
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.core.current().text();
        let (point, marked_len) = self.ime_point();
        let marked: Vec<u16> = self.marked.iter().flat_map(|m| m.encode_utf16()).collect();
        let len = text.len_utf16_cu() + marked_len;
        let (start, end) = (
            range.start.min(len),
            range.end.min(len).max(range.start.min(len)),
        );
        *adjusted = Some(start..end);
        let buffer = |a: usize, b: usize| {
            text.slice(text.utf16_cu_to_char(a)..text.utf16_cu_to_char(b))
                .to_string()
        };
        let mut out = String::new();
        if start < point {
            out += &buffer(start, end.min(point));
        }
        let (ms, me) = (
            start.clamp(point, point + marked_len),
            end.clamp(point, point + marked_len),
        );
        out += &String::from_utf16_lossy(&marked[ms - point..me - point]);
        if end > point + marked_len {
            out += &buffer(start.max(point + marked_len) - marked_len, end - marked_len);
        }
        Some(out)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let (point, marked) = self.ime_point();
        Some(UTF16Selection {
            range: point + marked..point + marked,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        let (point, marked) = self.ime_point();
        self.marked.as_ref().map(|_| point..point + marked)
    }

    /// Accepts the marked text as typed.
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.marked.take() {
            let replace = None;
            self.handle(Msg::Insert { text, replace }, cx);
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let replace = self.buffer_range(range);
        self.marked = None;
        let text = text.to_string();
        self.handle(Msg::Insert { text, replace }, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(replace) = self.buffer_range(range) {
            let text = String::new();
            let replace = Some(replace);
            self.handle(Msg::Insert { text, replace }, cx);
        }
        self.marked = Some(text.to_string()).filter(|t| !t.is_empty());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // shortcut: the cursor's box whatever the range, enough to place an IME's window; lay out the range if an IME needs it.
        self.painted.iter().find(|p| p.selected)?.cursor
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

/// The key `ks` is for the core, or `None` for one that types text, which the platform's text
/// input handles so dead keys and input methods work.
pub(crate) fn to_key(ks: &Keystroke, option_as_meta: bool) -> Option<Key> {
    let m = ks.modifiers;
    // shortcut: Key has no Fn modifier, so Fn keys are dropped; add one when a binding needs it.
    if m.function {
        return None;
    }
    let meta = m.alt && (option_as_meta || !cfg!(target_os = "macos"));
    let code = match ks.key.as_str() {
        "enter" => KeyCode::Return,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "escape" => KeyCode::Escape,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        // The key's own character, so C-x stays C-x whatever Option would type.
        _ if m.control || m.platform || meta || ks.key_char.is_none() => {
            let name = if ks.key == "space" { " " } else { &ks.key };
            let mut chars = name.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => KeyCode::Char(c),
                _ => return None,
            }
        }
        _ => return None,
    };
    Some(Key {
        code,
        ctrl: m.control,
        meta,
        shift: m.shift,
        super_: m.platform,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(s: &str, option_as_meta: bool) -> Option<String> {
        to_key(&Keystroke::parse(s).unwrap(), option_as_meta).map(|k| k.to_string())
    }

    #[test]
    fn sorts_command_keys_from_typing() {
        for (stroke, want) in [
            ("ctrl-x", Some("C-x")),
            ("cmd-z", Some("s-z")),
            ("cmd-shift-z", Some("S-s-z")),
            ("ctrl-space", Some("C-SPC")),
            ("alt-backspace", Some("M-DEL")),
            ("enter", Some("RET")),
            ("alt-enter", Some("M-RET")),
            ("alt-,", Some("M-,")),
            ("ctrl-alt-,", Some("C-M-,")),
            ("left", Some("<left>")),
            ("f1", None),
            ("fn-a", None),
        ] {
            assert_eq!(key(stroke, true).as_deref(), want, "{stroke}");
        }
        // A key that types, with the text the platform reports for it.
        let typed = |stroke: &str, text: &str| Keystroke {
            key_char: Some(text.into()),
            ..Keystroke::parse(stroke).unwrap()
        };
        assert_eq!(to_key(&typed("a", "a"), true), None);
        assert_eq!(to_key(&typed("shift-a", "A"), true), None);
        // Option for characters is macOS only; Linux's Alt stays Meta. macOS reports Option-e's
        // dead key as the accent and a no-break space.
        let alt_e = if cfg!(target_os = "macos") {
            None
        } else {
            Some("M-e".to_string())
        };
        let option_e = typed("alt-e", "\u{b4}\u{a0}");
        assert_eq!(to_key(&option_e, false).map(|k| k.to_string()), alt_e);
        assert_eq!(key("alt-e", true).as_deref(), Some("M-e"));
    }
}
