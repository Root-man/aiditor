//! The terminal emulator behind a small interface, so it can move off the UI thread later.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, test::TermSize};
use alacritty_terminal::vte::ansi::Processor;

use crate::buffer::{Buffer, BufferId};
use crate::window::WindowId;
use crate::{Core, Ctx, Effect, Effects, TerminalId, TerminalOp};

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

    /// The grid's rows and columns.
    pub fn size(&self) -> (u16, u16) {
        (self.term.screen_lines() as u16, self.term.columns() as u16)
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        (self.term).resize(TermSize::new(cols.max(1).into(), rows.max(1).into()));
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

impl Ctx<'_> {
    /// Spawns the login shell in a new `vterm<n><workspace>` shown in the selected window, in the
    /// current file's directory or the project's primary root.
    pub(crate) fn vterm(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let workspace = core.current_workspace();
        let name = (1..)
            .map(|n| format!("vterm{n}<{}>", workspace.name))
            .find(|name| core.buffers.find(name).is_none())
            .expect("a free number");
        let cwd = match core.current().file.as_deref().and_then(Path::parent) {
            Some(dir) => dir.to_path_buf(),
            None => (core.projects[core.project_at(workspace.project)?]
                .project
                .primary
                .path)
                .clone(),
        };
        let selected = core.windows.selected;
        let (rows, cols) = (core.window_cells().into_iter())
            .find(|&(id, ..)| id == selected)
            .map_or((24, 80), |(_, _, rows, cols)| (rows, cols));
        let (id, buffer) = core.create_terminal(&name, rows, cols);
        core.switch_to(buffer);
        self.effects.push(Effect::Terminal(TerminalOp::Spawn {
            id,
            command: None,
            cwd,
            env: Vec::new(),
            rows,
            cols,
        }));
        Ok(())
    }

    /// Kills terminal `id`'s buffer after its shell exited or failed to start, echoing `err`.
    pub(crate) fn end_terminal(&mut self, id: TerminalId, err: Option<String>) {
        let core = &mut *self.core;
        // Removed first, so `sync_terminals` sends no kill to a shell that is gone.
        if let Some(buffer) = core.terminals.remove(&id)
            && core.buffers.get(buffer).is_some()
        {
            core.kill_buffer(buffer);
            if let Some(err) = err {
                core.echo(err);
            }
        }
    }
}

impl Core {
    /// Whether buffer `id` is a terminal whose shell hasn't exited.
    pub(crate) fn live_terminal(&self, id: BufferId) -> bool {
        self.terminals.values().any(|&b| b == id)
    }

    /// Each laid-out window with its buffer and its text area's rows and columns; none before
    /// the frontend reports the frame size.
    fn window_cells(&self) -> Vec<(WindowId, BufferId, u16, u16)> {
        let (width, height, cell_width) = self.frame_size;
        if width <= 0. || height <= 0. || cell_width <= 0. {
            return Vec::new();
        }
        let line = self.settings.line_height();
        let frame = crate::display::Rect {
            x: 0.,
            y: 0.,
            width,
            height,
        };
        (self
            .windows
            .tree
            .layout(frame, (cell_width, line))
            .into_iter())
        .map(|(id, rect)| {
            let window = match id == self.windows.selected {
                true => &self.windows.window,
                false => &self.windows.others[&id],
            };
            // Less the mode line, as `window_display` counts.
            let rows = ((rect.height / line) as u16).saturating_sub(1).max(1);
            let cols = ((rect.width / cell_width) as u16).max(1);
            (id, window.buffer(), rows, cols)
        })
        .collect()
    }

    /// Kills the shells of killed terminal buffers, then sizes each terminal to the smallest
    /// window showing it, as Emacs's `window-adjust-process-window-size-smallest`.
    pub(crate) fn sync_terminals(&mut self, effects: &mut Effects) {
        let buffers = &self.buffers;
        self.terminals.retain(|&id, &mut buffer| {
            let live = buffers.get(buffer).is_some();
            if !live {
                effects.push(Effect::Terminal(TerminalOp::Kill { id }));
            }
            live
        });
        let mut sizes: BTreeMap<BufferId, (u16, u16)> = BTreeMap::new();
        for (_, buffer, rows, cols) in self.window_cells() {
            let size = sizes.entry(buffer).or_insert((rows, cols));
            *size = (size.0.min(rows), size.1.min(cols));
        }
        for (&id, buffer) in &self.terminals {
            let Some(&(rows, cols)) = sizes.get(buffer) else {
                continue;
            };
            let terminal = (self.buffers.get_mut(*buffer))
                .and_then(Buffer::terminal_mut)
                .expect("a live terminal's buffer");
            if terminal.size() != (rows, cols) {
                terminal.resize(rows, cols);
                effects.push(Effect::Terminal(TerminalOp::Resize { id, rows, cols }));
            }
        }
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
            assert!(h.effects.is_empty());
            h.core.buffers.kill(buffer).unwrap();
            output(&mut h, id, "\x1b[6n");
            // No reply; only the kill of the killed buffer's shell.
            assert!(
                matches!(&h.effects[..], [Effect::Terminal(TerminalOp::Kill { id: i })] if *i == id)
            );
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

    mod buffers {
        use crate::harness::{Harness, listed, name};
        use crate::{Msg, TerminalId, TerminalOp};

        /// The terminal work asked for since the last call, briefly.
        fn ops(h: &mut Harness) -> Vec<String> {
            (h.terminal_ops.drain(..))
                .map(|op| match op {
                    TerminalOp::Spawn {
                        id,
                        command: None,
                        cwd,
                        rows,
                        cols,
                        ..
                    } => format!("spawn {} {} {rows}x{cols}", id.0, cwd.display()),
                    TerminalOp::Spawn { .. } => "spawn a command".into(),
                    TerminalOp::Write { id, .. } => format!("write {}", id.0),
                    TerminalOp::Resize { id, rows, cols } => {
                        format!("resize {} {rows}x{cols}", id.0)
                    }
                    TerminalOp::Kill { id } => format!("kill {}", id.0),
                })
                .collect()
        }

        fn exists(h: &Harness, name: &str) -> bool {
            h.core.buffers.find(name).is_some()
        }

        /// A harness in workspace `work`, whose project's primary root is `/aiditor-test/app`.
        fn work() -> Harness {
            let mut h = Harness::new(&[("app/src/main.rs", "fn main() {}\n")]);
            h.core.rename_workspace("work").unwrap();
            h.core.projects[0].project.primary.path = h.path("app");
            h
        }

        #[test]
        fn names_count_within_a_workspace() {
            let mut h = work();
            h.keys("M-x vterm RET M-x vterm RET");
            assert_eq!(name(&h), "vterm2<work>");
            assert!(exists(&h, "vterm1<work>"));
            h.new_workspace("fix", "home");
            h.keys("M-x vterm RET");
            assert_eq!(name(&h), "vterm1<fix>");
            let one = h.core.buffers.find("vterm1<work>").unwrap();
            h.core.kill_buffer(one);
            h.core.switch_workspace(h.core.workspaces()[0].id).unwrap();
            h.keys("M-x vterm RET");
            assert_eq!(name(&h), "vterm1<work>");
        }

        #[test]
        fn the_shell_starts_in_the_files_directory_or_the_primary_root() {
            let mut h = work();
            h.open("app/src/main.rs").keys("M-x vterm RET");
            let src = h.path("app/src");
            assert_eq!(ops(&mut h), [format!("spawn 0 {} 24x80", src.display())]);
            h.keys("C-x b *scratch*<work> RET M-x vterm RET");
            let app = h.path("app");
            assert_eq!(ops(&mut h), [format!("spawn 1 {} 24x80", app.display())]);
        }

        #[test]
        fn the_terminal_follows_its_smallest_window() {
            let mut h = work();
            let line = h.core.settings().line_height();
            // 80 cells by 25.5 lines: 24 rows under the mode line.
            h.frame(640., 25.5 * line);
            h.keys("M-x vterm RET");
            let app = h.path("app");
            assert_eq!(ops(&mut h), [format!("spawn 0 {} 24x80", app.display())]);
            let size = |h: &Harness| {
                let buffer = h.core.buffers.find("vterm1<work>").unwrap();
                h.core
                    .buffers
                    .get(buffer)
                    .unwrap()
                    .terminal()
                    .unwrap()
                    .size()
            };
            assert_eq!(size(&h), (24, 80));
            // 13 and 12.5 lines; the lower window's 11 rows win.
            h.keys("C-x 2");
            assert_eq!(ops(&mut h), ["resize 0 11x80"]);
            h.frame(320., 25.5 * line);
            assert_eq!(ops(&mut h), ["resize 0 11x40"]);
            h.frame(320., 25.5 * line);
            assert!(ops(&mut h).is_empty());
            assert_eq!(size(&h), (11, 40));
            h.keys("C-x 1");
            assert_eq!(ops(&mut h), ["resize 0 24x40"]);
            // Shown in no window, it keeps its size.
            h.keys("C-x b RET");
            h.frame(640., 10. * line);
            assert!(ops(&mut h).is_empty());
        }

        #[test]
        fn the_buffer_follows_the_shell() {
            let mut h = work();
            h.keys("M-x vterm RET M-x vterm RET");
            ops(&mut h);
            h.send(Msg::TerminalExited {
                id: TerminalId(0),
                code: 0,
                signal: None,
            });
            assert!(!exists(&h, "vterm1<work>"));
            h.send(Msg::TerminalSpawned {
                id: TerminalId(1),
                result: Err("/x: not a directory".into()),
            });
            assert!(!exists(&h, "vterm2<work>"));
            assert_eq!(h.core.message(), Some("/x: not a directory"));
            assert!(ops(&mut h).is_empty());
            assert_eq!(name(&h), "*scratch*<work>");
        }

        #[test]
        fn killing_a_workspace_kills_its_shells() {
            let mut h = work();
            h.new_workspace("fix", "home");
            h.keys("M-x vterm RET M-x vterm RET");
            ops(&mut h);
            let fix = h.core.workspace();
            h.core.kill_workspace(fix).unwrap();
            h.send(Msg::Tick);
            assert_eq!(ops(&mut h), ["kill 0", "kill 1"]);
        }

        #[test]
        fn killing_a_live_terminal_asks_first() {
            let mut h = work();
            h.keys("M-x vterm RET C-x k RET");
            ops(&mut h);
            assert_eq!(
                h.core.minibuffer().unwrap().prompt,
                "Buffer vterm1<work> has a running process; kill it? "
            );
            assert_eq!(listed(&mut h).0, ["no", "yes"]);
            h.keys("RET");
            assert_eq!(name(&h), "vterm1<work>");
            assert!(ops(&mut h).is_empty());
            h.keys("C-x k RET yes RET");
            assert!(!exists(&h, "vterm1<work>"));
            assert_eq!(ops(&mut h), ["kill 0"]);
            // The exit that follows the kill changes nothing.
            h.send(Msg::TerminalExited {
                id: TerminalId(0),
                code: 0,
                signal: Some("SIGHUP".into()),
            });
            assert!(ops(&mut h).is_empty());
        }

        #[test]
        fn quitting_asks_about_live_terminals() {
            let mut h = work();
            h.keys("M-x vterm RET C-x C-c");
            assert_eq!(
                h.core.minibuffer().unwrap().prompt,
                "Active processes exist; kill them and exit anyway? "
            );
            assert_eq!(listed(&mut h).0, ["No", "Yes"]);
            h.keys("RET");
            assert!(!h.quit && h.core.minibuffer().is_none());
            assert!(exists(&h, "vterm1<work>"));
            assert!(ops(&mut h)[1..].is_empty(), "only the spawn");
            h.keys("C-x C-c yes RET");
            assert!(h.quit);
            // The app kills every terminal on quit; the core sends nothing itself.
            assert!(ops(&mut h).is_empty());
        }

        #[test]
        fn quitting_asks_about_terminals_after_the_saves() {
            for (answer, quit) in [("RET", false), ("yes RET", true)] {
                let mut h = work();
                h.keys("M-x vterm RET");
                h.open("app/src/main.rs").keys("x C-x C-c RET");
                assert_eq!(
                    h.core.minibuffer().unwrap().prompt,
                    "Active processes exist; kill them and exit anyway? "
                );
                h.save_reply("xfn main() {}\n");
                h.keys(answer);
                // Declining still saves the buffer answered `Yes`.
                assert_eq!(h.submitted("app/src/main.rs"), "xfn main() {}\n");
                assert_eq!(h.quit, quit, "{answer}");
            }
        }
    }
}
