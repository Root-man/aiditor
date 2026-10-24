//! The terminal emulator behind a small interface, so it can move off the UI thread later.

use std::cell::RefCell;
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, test::TermSize};
use alacritty_terminal::vte::ansi::Processor;

/// Collects what the emulator writes back to the program; every other event is dropped.
#[derive(Clone, Default)]
struct Replies(Rc<RefCell<Vec<u8>>>);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            self.0.borrow_mut().extend_from_slice(text.as_bytes());
        }
    }
}

/// One terminal's grid and the parser state between output batches.
pub struct Terminal {
    term: Term<Replies>,
    parser: Processor,
    replies: Replies,
}

impl Terminal {
    pub fn new(rows: u16, cols: u16) -> Self {
        let replies = Replies::default();
        let size = TermSize::new(cols.max(1).into(), rows.max(1).into());
        Terminal {
            term: Term::new(Config::default(), &size, replies.clone()),
            parser: Processor::new(),
            replies,
        }
    }

    /// Feeds the program's output into the grid; returns the replies it asked for, in order.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.parser.advance(&mut self.term, bytes);
        std::mem::take(&mut *self.replies.0.borrow_mut())
    }

    /// The visible rows, trailing blanks trimmed, joined by `\n`.
    pub fn screen(&self) -> String {
        let rows: Vec<String> = (0..self.term.screen_lines() as i32)
            .map(|line| self.row(line))
            .collect();
        rows.join("\n")
    }

    /// Lines in the scrollback, above the screen.
    pub fn history(&self) -> usize {
        self.term.grid().history_size()
    }

    /// Row `line`'s text, trailing blanks trimmed; 0 is the screen's top, negative is scrollback.
    pub fn row(&self, line: i32) -> String {
        let row = &self.term.grid()[Line(line)];
        let mut text = String::new();
        for col in 0..self.term.columns() {
            let cell = &row[Column(col)];
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            text.push(cell.c);
            text.extend(cell.zerowidth().into_iter().flatten());
        }
        text.truncate(text.trim_end_matches(' ').len());
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fed(rows: u16, cols: u16, bytes: &str) -> Terminal {
        let mut t = Terminal::new(rows, cols);
        assert_eq!(t.feed(bytes.as_bytes()), b"");
        t
    }

    #[test]
    fn plain_lines() {
        assert_eq!(fed(4, 10, "one\r\ntwo\r\n").screen(), "one\ntwo\n\n");
    }

    #[test]
    fn cursor_moves_and_clears() {
        let mut t = fed(3, 10, "hello\x1b[1;1Hj");
        assert_eq!(t.screen(), "jello\n\n");
        t.feed(b"\x1b[2J\x1b[Hnew");
        assert_eq!(t.screen(), "new\n\n");
    }

    #[test]
    fn long_lines_wrap() {
        assert_eq!(fed(2, 4, "abcdef").screen(), "abcd\nef");
    }

    #[test]
    fn scrolled_lines_go_to_the_scrollback() {
        let t = fed(3, 10, "1\r\n2\r\n3\r\n4\r\n5");
        assert_eq!(t.screen(), "3\n4\n5");
        assert_eq!(
            (t.history(), t.row(-2), t.row(-1)),
            (2, "1".into(), "2".into())
        );
    }

    #[test]
    fn a_sequence_split_across_batches() {
        let mut t = fed(2, 10, "ab\x1b[1");
        t.feed(b";1Hx");
        assert_eq!(t.screen(), "xb\n");
    }

    #[test]
    fn wide_chars_take_one_char() {
        assert_eq!(fed(1, 10, "日本x").screen(), "日本x");
    }

    #[test]
    fn reports_are_replies() {
        let mut t = Terminal::new(3, 10);
        assert_eq!(t.feed(b"ab\x1b[6n"), b"\x1b[1;3R");
        assert!(t.feed(b"\x1b[c").starts_with(b"\x1b[?"));
    }

    #[test]
    fn title_bell_and_clipboard_are_ignored() {
        fed(
            3,
            10,
            "\x1b]0;title\x07\x07\x1b]52;c;aGk=\x07\x1b]52;c;?\x07",
        );
    }

    mod in_the_core {
        use crate::harness::Harness;
        use crate::{Effect, Msg, TerminalId, TerminalOp};

        fn output(h: &mut Harness, id: TerminalId, bytes: &str) {
            h.send(Msg::TerminalOutput {
                id,
                bytes: bytes.into(),
            });
        }

        fn writes(h: &mut Harness) -> Vec<(TerminalId, Vec<u8>)> {
            (h.effects.drain(..))
                .filter_map(|e| match e {
                    Effect::Terminal(TerminalOp::Write { id, bytes }) => Some((id, bytes)),
                    _ => None,
                })
                .collect()
        }

        #[test]
        fn output_reaches_its_buffers_grid() {
            let mut h = Harness::new(&[]);
            let (one, a) = h.core.create_terminal("*terminal*", 2, 10);
            let (two, b) = h.core.create_terminal("*terminal*", 2, 10);
            output(&mut h, one, "one");
            output(&mut h, two, "two\x1b[2J\x1b[Hdos");
            let screen = |id| h.core.buffers.get(id).unwrap().terminal().unwrap().screen();
            assert_eq!((screen(a), screen(b)), ("one\n".into(), "dos\n".into()));
            assert_eq!(h.core.buffers.get(b).unwrap().name, "*terminal*<2>");
        }

        #[test]
        fn replies_go_back_as_writes() {
            let mut h = Harness::new(&[]);
            let (_, _) = h.core.create_terminal("a", 2, 10);
            let (id, _) = h.core.create_terminal("b", 2, 10);
            h.collect_effects = true;
            output(&mut h, id, "ab\x1b[6n");
            assert_eq!(writes(&mut h), [(id, b"\x1b[1;3R".to_vec())]);
            output(&mut h, id, "\x1b[c");
            assert!(
                matches!(&writes(&mut h)[..], [(i, b)] if *i == id && b.starts_with(b"\x1b[?"))
            );
            output(&mut h, id, "plain\x1b]0;title\x07\x07\x1b]52;c;aGk=\x07");
            assert!(h.effects.is_empty());
        }

        #[test]
        fn output_for_an_unknown_or_killed_terminal_is_ignored() {
            let mut h = Harness::new(&[]);
            let (id, buffer) = h.core.create_terminal("t", 2, 10);
            h.collect_effects = true;
            output(&mut h, TerminalId(99), "\x1b[6n");
            h.core.buffers.kill(buffer).unwrap();
            output(&mut h, id, "\x1b[6n");
            assert!(h.effects.is_empty());
        }

        #[test]
        fn text_commands_are_harmless_and_edits_fail() {
            let mut h = Harness::new(&[]);
            let (id, buffer) = h.core.create_terminal("t", 2, 10);
            output(&mut h, id, "shown");
            h.core.show_buffer(buffer);
            h.keys("C-f C-n C-b M-> M-< C-x h C-g C-s shown C-g");
            assert_eq!((h.point(), h.text()), (0, String::new()));
            h.keys("x");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            h.clipboard = Some("pasted".into());
            h.keys("C-g C-y");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            let terminal = h.core.buffers.get(buffer).unwrap().terminal().unwrap();
            assert_eq!(terminal.screen(), "shown\n");
        }
    }
}
