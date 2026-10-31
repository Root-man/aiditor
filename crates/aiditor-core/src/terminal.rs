//! The terminal emulator behind a small interface, so it can move off the UI thread later.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::ops::Range;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermMode, test::TermSize};
use alacritty_terminal::vte::ansi::{Color, CursorShape as Shape, NamedColor, Processor, Rgb};
use alacritty_terminal::vte::{Params, Parser, Perform};

use crate::buffer::{Buffer, BufferId, Face, Prop};
use crate::display::{self, CursorShape, Run, TEXT_INSET};
use crate::key::{Key, KeyCode};
use crate::settings::Theme;
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

/// What `Terminal::copy_text` returns: text, faces, the cursor's line and column, and commands.
pub type CopyText = (
    String,
    Vec<(Range<usize>, Face)>,
    usize,
    usize,
    Vec<ShellCommand>,
);

/// A command's OSC 133 marks as char indices in copy mode's text.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellCommand {
    /// Its `A` mark, else its `B`.
    pub start: usize,
    /// Its `B` mark, else its `A`.
    pub input: usize,
    pub output: Option<usize>,
    pub end: Option<usize>,
}

/// Lines of scrollback, alacritty's default.
const SCROLLBACK: usize = 10_000;

/// An OSC 133 shell integration mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Prompt,
    Input,
    Output,
    /// The command's end, with its exit code when the shell gave one.
    End(Option<i32>),
}

/// What `Scan` stops the batch at, to apply once alacritty has seen the bytes before it.
enum Signal {
    /// A mark, with the command line a `C` mark gave.
    Mark(Mark, Option<String>),
    Cwd(PathBuf),
    ClearHistory,
    Reset,
}

/// Finds the sequences alacritty drops or that move the marks; parses nothing else.
#[derive(Default)]
struct Scan {
    signal: Option<Signal>,
}

impl Perform for Scan {
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        self.signal = match params {
            [b"133", kind, rest @ ..] => {
                let mark = match *kind {
                    b"A" => Mark::Prompt,
                    b"B" => Mark::Input,
                    b"C" => Mark::Output,
                    b"D" => Mark::End(
                        (rest.first())
                            .and_then(|code| std::str::from_utf8(code).ok()?.parse().ok()),
                    ),
                    _ => return,
                };
                let command = (rest.iter())
                    .find_map(|field| field.strip_prefix(b"cmdline_url="))
                    .and_then(percent_decoded);
                let command = command.map(|text| String::from_utf8_lossy(&text).into_owned());
                Some(Signal::Mark(mark, command))
            }
            [b"7", url @ ..] => local_dir(&url.join(&b';')).map(Signal::Cwd),
            _ => None,
        };
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _: bool, action: char) {
        if action == 'J' && intermediates.is_empty() && params.iter().next() == Some(&[3]) {
            self.signal = Some(Signal::ClearHistory);
        }
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _: bool, byte: u8) {
        if byte == b'c' && intermediates.is_empty() {
            self.signal = Some(Signal::Reset);
        }
    }

    fn terminated(&self) -> bool {
        self.signal.is_some()
    }
}

/// OSC 7's `file://host/path` or `kitty-shell-cwd://host/path`, when the host is this machine.
fn local_dir(url: &[u8]) -> Option<PathBuf> {
    let rest =
        (url.strip_prefix(b"file://")).or_else(|| url.strip_prefix(b"kitty-shell-cwd://"))?;
    let (host, path) = rest.split_at(rest.iter().position(|&b| b == b'/')?);
    let local = host.is_empty()
        || host.eq_ignore_ascii_case(b"localhost")
        || host.eq_ignore_ascii_case(&hostname());
    let decoded = percent_decoded(path)?;
    (local && !decoded.contains(&0)).then(|| OsString::from_vec(decoded).into())
}

/// `text` with its `%XX` escapes decoded; `None` if one is malformed.
fn percent_decoded(text: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        if text[i] == b'%' {
            let hex = text
                .get(i + 1..i + 3)
                .filter(|h| h.iter().all(u8::is_ascii_hexdigit))?;
            decoded.push(u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?);
            i += 3;
        } else {
            decoded.push(text[i]);
            i += 1;
        }
    }
    Some(decoded)
}

fn hostname() -> Vec<u8> {
    let mut name = [0u8; 256];
    // SAFETY: the buffer outlives the call, which writes at most its length.
    if unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) } != 0 {
        return Vec::new();
    }
    name.split(|&b| b == 0).next().unwrap_or_default().to_vec()
}

/// One terminal's grid and the parser state between output batches.
pub struct Terminal {
    term: Term<Replies>,
    parser: Processor,
    replies: Replies,
    scan_parser: Parser,
    scan: Scan,
    /// Each mark with its absolute line (the scrollback's oldest is 0) and column.
    marks: Vec<(usize, usize, Mark)>,
    cwd: Option<PathBuf>,
    /// The command between its `C` and `D` marks, kept apart from the marks as those stop at a
    /// full scrollback and on the alternate screen, where a dying program leaves zsh's `D`.
    running: Option<String>,
    /// The last command's exit status, when it failed.
    failed: Option<i32>,
}

impl Terminal {
    pub fn new(rows: u16, cols: u16) -> Self {
        let replies = Replies::default();
        let size = TermSize::new(cols.max(1).into(), rows.max(1).into());
        let config = Config {
            scrolling_history: SCROLLBACK,
            ..Config::default()
        };
        Terminal {
            term: Term::new(config, &size, replies.clone()),
            parser: Processor::new(),
            replies,
            scan_parser: Parser::new(),
            scan: Scan::default(),
            marks: Vec::new(),
            cwd: None,
            running: None,
            failed: None,
        }
    }

    /// Feeds the program's output into the grid; returns the replies it asked for, in order.
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut rest = bytes;
        while !rest.is_empty() {
            // Both parsers see the same chunks, so their states stay in step across batches.
            // shortcut: inside a synchronized update (CSI ? 2026 h) alacritty holds bytes back, so a
            // mark there takes the cursor before the update; flush the sync first if shells do that.
            let n = (self.scan_parser).advance_until_terminated(&mut self.scan, rest);
            self.parser.advance(&mut self.term, &rest[..n]);
            rest = &rest[n..];
            let main = !self.term.mode().contains(TermMode::ALT_SCREEN);
            // shortcut: alacritty stops counting lines dropped from a full scrollback, so marks
            // stop there; count them (as a ring offset) if marks must outlive 10,000 lines.
            let full = main && self.term.grid().history_size() >= SCROLLBACK;
            match self.scan.signal.take() {
                Some(Signal::Mark(mark, command)) => {
                    match mark {
                        Mark::Prompt => self.running = None,
                        Mark::Output => (self.running, self.failed) = (command, None),
                        Mark::End(code) => {
                            (self.running, self.failed) = (None, code.filter(|&c| c != 0));
                        }
                        Mark::Input => {}
                    }
                    if main && !full {
                        let grid = self.term.grid();
                        let point = grid.cursor.point;
                        let line = grid.history_size() + point.line.0 as usize;
                        self.marks.push((line, point.column.0, mark));
                    }
                }
                Some(Signal::Cwd(path)) => self.cwd = Some(path),
                Some(Signal::ClearHistory) if main => self.marks.clear(),
                Some(Signal::Reset) => self.marks.clear(),
                _ => {}
            }
            if full {
                self.marks.clear();
            }
        }
        std::mem::take(&mut *self.replies.0.borrow_mut())
    }

    /// The OSC 133 marks kept so far, oldest first, each with its absolute line (`history()`
    /// plus a screen row) and grid column.
    // shortcut: a column resize reflows wrapped lines without moving the marks, so a mark below
    // a rewrapped line is off by the rows it gained or lost; track them through reflow if that
    // matters.
    pub fn marks(&self) -> &[(usize, usize, Mark)] {
        &self.marks
    }

    /// The shell's directory, from its last local OSC 7, else the one `vterm` spawned it in.
    pub fn cwd(&self) -> Option<&Path> {
        self.cwd.as_deref()
    }

    /// The running command as the shell gave it, if it did.
    pub fn running(&self) -> Option<&str> {
        self.running.as_deref()
    }

    /// The last command's exit status, when it failed.
    pub fn failed(&self) -> Option<i32> {
        self.failed
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
        let mut text = self.cells(line, self.term.columns());
        text.truncate(text.trim_end_matches(' ').len());
        text
    }

    /// The text of row `line`'s cells before column `end`, untrimmed.
    fn cells(&self, line: i32, end: usize) -> String {
        let row = &self.term.grid()[Line(line)];
        let mut text = String::new();
        for col in 0..end {
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
        text
    }

    /// Copy mode's text: the scrollback then the screen, soft-wrapped rows joined and blank rows
    /// below the cursor dropped; the char ranges of its colours resolved against `theme`; the
    /// cursor's line and char column in it; and each command's marks, sorted by start.
    pub fn copy_text(&self, theme: &Theme) -> CopyText {
        let grid = self.term.grid();
        let (cursor, cols) = (grid.cursor.point, self.term.columns());
        let mut lines = vec![String::new()];
        // Each colour as its line and char range in that line.
        let mut colors = Vec::new();
        let mut at = (0, 0);
        let mut on_line: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, &(line, ..)) in self.marks.iter().enumerate() {
            on_line.entry(line).or_default().push(i);
        }
        let mut places = vec![None; self.marks.len()];
        // The current logical line's length in chars, kept so wrapped rows aren't recounted.
        let mut offset = 0;
        for line in -(self.history() as i32)..self.term.screen_lines() as i32 {
            let n = lines.len() - 1;
            let current = &mut lines[n];
            if line == cursor.line.0 {
                let before = self.cells(line, cursor.column.0);
                at = (n, offset + before.chars().count());
            }
            let absolute = (line + self.history() as i32) as usize;
            for &i in on_line.get(&absolute).into_iter().flatten() {
                let before = self.cells(line, self.marks[i].1.min(cols));
                places[i] = Some((n, offset + before.chars().count()));
            }
            for run in self.runs(line, theme) {
                let face = Face {
                    fg: Some(run.fg),
                    bg: run.bg,
                    bold: run.bold,
                    italic: run.italic,
                    underline: run.underline,
                };
                if face
                    != (Face {
                        fg: Some(theme.foreground()),
                        ..Face::default()
                    })
                {
                    let start = offset + self.cells(line, run.col.into()).chars().count();
                    colors.push((n, start..start + run.text.chars().count(), face));
                }
            }
            if grid[Line(line)][Column(cols - 1)]
                .flags
                .contains(Flags::WRAPLINE)
            {
                let row = self.cells(line, cols);
                offset += row.chars().count();
                *current += &row;
            } else {
                *current += &self.row(line);
                lines.push(String::new());
                offset = 0;
            }
        }
        while lines.len() > at.0 + 1 && lines.last().is_some_and(String::is_empty) {
            lines.pop();
        }
        let mut starts = vec![0];
        for line in &lines {
            starts.push(starts.last().expect("a start") + line.chars().count() + 1);
        }
        let mut commands: Vec<ShellCommand> = Vec::new();
        let mut awaits_input = false;
        for (&(.., mark), place) in self.marks.iter().zip(places) {
            let at = (place.filter(|&(n, _)| n < lines.len()))
                .map(|(n, col)| (starts[n] + col).min(starts[n + 1] - 1));
            let last = commands.last_mut();
            match (mark, at) {
                (Mark::Input, Some(at)) if awaits_input => last.expect("a prompt").input = at,
                (Mark::Prompt | Mark::Input, Some(at)) => commands.push(ShellCommand {
                    start: at,
                    input: at,
                    output: None,
                    end: None,
                }),
                (Mark::Output, Some(at)) => {
                    if let Some(last) = last {
                        last.output.get_or_insert(at);
                    }
                }
                // A `D` without a `C` ends an empty command line.
                (Mark::End(_), Some(at)) => {
                    if let Some(last) = last.filter(|c| c.output.is_some()) {
                        last.end.get_or_insert(at);
                    }
                }
                _ => {}
            }
            // A prompt with no place of its own leaves its input to stand alone.
            awaits_input = mark == Mark::Prompt && at.is_some();
        }
        commands.sort_by_key(|c| c.start);
        // Colour on trimmed blanks or dropped rows has no text to go on.
        let faces = (colors.into_iter())
            .filter(|(n, ..)| *n < lines.len())
            .map(|(n, range, face)| {
                let end = starts[n + 1] - 1;
                (
                    (starts[n] + range.start).min(end)..(starts[n] + range.end).min(end),
                    face,
                )
            })
            .filter(|(range, _)| !range.is_empty())
            .collect();
        (lines.join("\n"), faces, at.0, at.1, commands)
    }

    /// The screen's rows as runs of styled cells, and the cursor, colors resolved against `theme`.
    pub fn display(&self, theme: &Theme) -> display::Terminal {
        let rows = (0..self.term.screen_lines() as i32).map(|line| self.runs(line, theme));
        let cursor = self.term.renderable_content().cursor;
        let shape = match cursor.shape {
            Shape::Hidden => None,
            Shape::Beam => Some(CursorShape::Beam),
            Shape::Underline => Some(CursorShape::Underline),
            Shape::Block | Shape::HollowBlock => Some(CursorShape::Block),
        };
        let wide = self.term.grid()[cursor.point]
            .flags
            .contains(Flags::WIDE_CHAR);
        display::Terminal {
            rows: rows.collect(),
            cursor: shape.map(|shape| display::Cursor {
                row: cursor.point.line.0 as u16,
                col: cursor.point.column.0 as u16,
                cells: if wide { 2 } else { 1 },
                shape,
            }),
        }
    }

    /// Row `line`'s runs of styled cells, colors resolved against `theme`.
    fn runs(&self, line: i32, theme: &Theme) -> Vec<Run> {
        let row = &self.term.grid()[Line(line)];
        let mut runs: Vec<Run> = Vec::new();
        for col in 0..self.term.columns() {
            let cell = &row[Column(col)];
            let flags = cell.flags;
            if flags.intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER) {
                continue;
            }
            let (mut fg, mut bg) = (self.color(cell.fg, theme), self.color(cell.bg, theme));
            if flags.contains(Flags::INVERSE) {
                (fg, bg) = (bg, fg);
            }
            if flags.contains(Flags::DIM) {
                fg = dim(fg);
            }
            let bg = (bg != theme.background()).then_some(bg);
            let (bold, italic) = (flags.contains(Flags::BOLD), flags.contains(Flags::ITALIC));
            let underline = flags.intersects(Flags::ALL_UNDERLINES);
            if cell.c == ' ' && bg.is_none() && !underline {
                continue;
            }
            let (col, wide) = (col as u16, flags.contains(Flags::WIDE_CHAR));
            let mut text = String::from(cell.c);
            text.extend(cell.zerowidth().into_iter().flatten());
            // A wide char runs alone, so each glyph of a run advances exactly one cell.
            match runs.last_mut() {
                Some(run)
                    if !wide
                        && run.col + run.cells == col
                        && run.cells as usize == run.text.chars().count()
                        && (run.fg, run.bg, run.bold, run.italic, run.underline)
                            == (fg, bg, bold, italic, underline) =>
                {
                    run.cells += 1;
                    run.text += &text;
                }
                _ => runs.push(Run {
                    col,
                    cells: if wide { 2 } else { 1 },
                    text,
                    fg,
                    bg,
                    bold,
                    italic,
                    underline,
                }),
            }
        }
        runs
    }

    /// The bytes xterm sends for `key`, as Zed's `keys.rs`, or `None` for a key it has none for.
    pub fn encode(&self, key: Key) -> Option<Vec<u8>> {
        let app_cursor = self.term.mode().contains(TermMode::APP_CURSOR);
        let m = 1 + u8::from(key.shift) + 2 * u8::from(key.meta) + 4 * u8::from(key.ctrl);
        let esc = |s: String| Some(format!("\x1b{s}").into_bytes());
        let cursor = |c: char| match m {
            1 if app_cursor => esc(format!("O{c}")),
            1 => esc(format!("[{c}")),
            _ => esc(format!("[1;{m}{c}")),
        };
        let tilde = |n: u8| match m {
            1 => esc(format!("[{n}~")),
            _ => esc(format!("[{n};{m}~")),
        };
        let bytes = match key.code {
            KeyCode::Left => return cursor('D'),
            KeyCode::Right => return cursor('C'),
            KeyCode::Up => return cursor('A'),
            KeyCode::Down => return cursor('B'),
            KeyCode::Home => return cursor('H'),
            KeyCode::End => return cursor('F'),
            KeyCode::PageUp => return tilde(5),
            KeyCode::PageDown => return tilde(6),
            KeyCode::Delete => return tilde(3),
            KeyCode::F(n @ 1..=4) if m == 1 => return esc(format!("O{}", (b'O' + n) as char)),
            KeyCode::F(n @ 1..=4) => return esc(format!("[1;{m}{}", (b'O' + n) as char)),
            KeyCode::F(n @ 5..=12) => {
                return tilde([15, 17, 18, 19, 20, 21, 23, 24][n as usize - 5]);
            }
            KeyCode::F(_) => return None,
            KeyCode::Tab if key.shift => b"\x1b[Z".to_vec(),
            KeyCode::Tab => b"\t".to_vec(),
            KeyCode::Return if key.shift => b"\n".to_vec(),
            KeyCode::Return => b"\r".to_vec(),
            KeyCode::Escape => b"\x1b".to_vec(),
            KeyCode::Backspace if key.ctrl => b"\x08".to_vec(),
            KeyCode::Backspace => b"\x7f".to_vec(),
            KeyCode::Char(' ') if key.ctrl => b"\0".to_vec(),
            KeyCode::Char('?') if key.ctrl => b"\x7f".to_vec(),
            KeyCode::Char(c @ ('@'..='_' | 'a'..='z')) if key.ctrl => {
                vec![c.to_ascii_uppercase() as u8 & 0x1f]
            }
            KeyCode::Char(_) if key.ctrl => return None,
            KeyCode::Char(c) if key.shift => c.to_uppercase().to_string().into_bytes(),
            KeyCode::Char(c) => c.to_string().into_bytes(),
        };
        Some(if key.meta {
            [b"\x1b", &bytes[..]].concat()
        } else {
            bytes
        })
    }

    /// The bytes that paste `text`: between bracketed-paste marks, without ESC, when the program
    /// turned on mode 2004, as Zed does, else as is.
    pub fn paste(&self, text: &str) -> Vec<u8> {
        if self.term.mode().contains(TermMode::BRACKETED_PASTE) {
            format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
        } else {
            text.as_bytes().to_vec()
        }
    }

    /// `color` as 0xRRGGBB: what the program set with OSC 4, 10 or 11, else the theme's palette,
    /// the xterm cube or gray ramp.
    fn color(&self, color: Color, theme: &Theme) -> u32 {
        let rgb = |c: Rgb| (c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32;
        let index = match color {
            Color::Spec(c) => return rgb(c),
            Color::Indexed(i) => i as usize,
            Color::Named(n) => n as usize,
        };
        if let Some(c) = self.term.colors()[index] {
            return rgb(c);
        }
        let level = |v: usize| if v == 0 { 0 } else { 55 + 40 * v as u32 };
        match color {
            Color::Named(NamedColor::Foreground | NamedColor::BrightForeground) => {
                theme.foreground()
            }
            Color::Named(NamedColor::Background) => theme.background(),
            Color::Named(NamedColor::Cursor) => theme.cursor(),
            Color::Named(NamedColor::DimForeground) => dim(theme.foreground()),
            Color::Named(_) if index > 256 => {
                dim(theme.ansi[index - NamedColor::DimBlack as usize])
            }
            _ => match index {
                0..16 => theme.ansi[index],
                16..232 => {
                    let i = index - 16;
                    level(i / 36) << 16 | level(i / 6 % 6) << 8 | level(i % 6)
                }
                _ => 0x010101 * (8 + 10 * (index - 232) as u32),
            },
        }
    }
}

/// `color` at 2/3 of each channel, as alacritty dims.
fn dim(color: u32) -> u32 {
    let channel = |shift: u32| ((color >> shift & 0xff) * 2 / 3) << shift;
    channel(16) | channel(8) | channel(0)
}

impl Ctx<'_> {
    /// Spawns the login shell in a new `vterm<n><workspace>` shown in the selected window, in the
    /// current buffer's directory or the project's primary root.
    pub(crate) fn vterm(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let workspace = core.current_workspace();
        let name = (1..)
            .map(|n| format!("vterm{n}<{}>", workspace.name))
            .find(|name| core.buffers.find(name).is_none())
            .expect("a free number");
        let project = &core.projects[core.project_at(workspace.project)?].project;
        let env = project.env.clone().into_iter().collect();
        let cwd = match core.buffer_directory() {
            Some(dir) => dir,
            None => project.primary.path.clone(),
        };
        let selected = core.windows.selected;
        let (rows, cols) = (core.window_cells().into_iter())
            .find(|&(id, ..)| id == selected)
            .map_or((24, 80), |(_, _, rows, cols)| (rows, cols));
        let (id, buffer) = core.create_terminal(&name, rows, cols);
        let terminal = core.buffers.get_mut(buffer).and_then(Buffer::terminal_mut);
        terminal.expect("a new terminal").cwd = Some(cwd.clone());
        core.switch_to(buffer);
        self.effects.push(Effect::Terminal(TerminalOp::Spawn {
            id,
            command: None,
            cwd,
            env,
            rows,
            cols,
        }));
        Ok(())
    }

    /// Sends `^C` to the current buffer's shell.
    pub(crate) fn vterm_send_c_c(&mut self) -> Result<(), String> {
        let id = self.core.char_mode().ok_or("Not in a live terminal")?;
        let bytes = b"\x03".to_vec();
        self.effects
            .push(Effect::Terminal(TerminalOp::Write { id, bytes }));
        Ok(())
    }

    /// Pastes the newest kill, or clipboard text from another app, into the current buffer's shell.
    pub(crate) fn vterm_yank(&mut self) -> Result<(), String> {
        let id = self.core.char_mode().ok_or("Not in a live terminal")?;
        self.core.kills.take_clipboard();
        let text = self.core.kills.current_kill(0)?;
        let terminal = self
            .core
            .current()
            .terminal()
            .expect("a live terminal's buffer");
        let bytes = terminal.paste(&text);
        self.effects
            .push(Effect::Terminal(TerminalOp::Write { id, bytes }));
        Ok(())
    }

    /// Enters copy mode in the current terminal buffer, with point at the terminal's cursor, or
    /// leaves it without copying.
    pub(crate) fn vterm_copy_mode(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let buffer =
            (core.buffers.get_mut(core.windows.window.buffer())).expect("current buffer is live");
        let terminal = buffer.terminal().ok_or("Not in a terminal")?;
        if buffer.copy_mode() {
            buffer.set_copy_text(None);
            core.windows.window.set_active(false);
            return Ok(());
        }
        let (text, faces, line, col, commands) = terminal.copy_text(&core.settings.theme);
        buffer.set_copy_text(Some(text.as_str().into()));
        for (range, face) in faces {
            buffer.add_property(range, Prop::Face(face))?;
        }
        buffer.commands = commands;
        let at = buffer.char_idx(line, col)?;
        core.windows.window.set_point(&mut core.buffers, at)?;
        Ok(())
    }

    /// Moves point to the `n`th command's input start after point, before it when `n` is
    /// negative, or the farthest one when fewer exist.
    pub(crate) fn vterm_next_prompt(&mut self, n: i64) -> Result<(), String> {
        let core = &mut *self.core;
        let point = core.point();
        let mut prompts: Vec<usize> = core.current().commands.iter().map(|c| c.input).collect();
        prompts.sort_unstable();
        prompts.dedup();
        let count = n.unsigned_abs() as usize;
        let target = match n > 0 {
            true => prompts.iter().filter(|&&p| p > point).take(count).last(),
            false => prompts
                .iter()
                .rev()
                .filter(|&&p| p < point)
                .take(count)
                .last(),
        };
        let at = *target.ok_or("No prompt found")?;
        core.windows.window.set_point(&mut core.buffers, at)?;
        Ok(())
    }

    /// Sets an active region over the output of the command at point, from its `C` mark to its
    /// `D`, else the next command's start, else the text's end, without its last newline.
    pub(crate) fn vterm_mark_output(&mut self) -> Result<(), String> {
        let core = &mut *self.core;
        let (buffer, point) = (core.current(), core.point());
        let commands = &buffer.commands;
        let i = (commands.iter().rposition(|c| c.start <= point)).ok_or("No command at point")?;
        let output = commands[i].output.ok_or("No command at point")?;
        let mut end = (commands[i].end)
            .or(commands.get(i + 1).map(|c| c.start))
            .unwrap_or(buffer.len_chars())
            .max(output);
        if end > output && buffer.text().get_char(end - 1) == Some('\n') {
            end -= 1;
        }
        let window = &mut core.windows.window;
        window.push_mark(&mut core.buffers, end, true)?;
        window.set_point(&mut core.buffers, output)?;
        Ok(())
    }

    /// Copies the region, or point's line without one, to the kill ring and the clipboard and
    /// leaves copy mode.
    pub(crate) fn vterm_copy_mode_done(&mut self) -> Result<(), String> {
        let core = &*self.core;
        let buffer = core.current();
        if !buffer.copy_mode() {
            return Err("Not in copy mode".into());
        }
        let (window, point) = (&core.windows.window, core.point());
        let range = match window.mark(&core.buffers).filter(|_| window.active()) {
            Some(mark) => point.min(mark)..point.max(mark),
            None => {
                let (line, _) = buffer.line_col(point)?;
                buffer.char_idx(line, 0)?..buffer.char_idx(line, usize::MAX)?
            }
        };
        self.kill_region(Some(range), true)?;
        self.vterm_copy_mode()
    }

    /// Kills terminal `id`'s buffer after its shell exited or failed to start, echoing `err`, and
    /// sends the terminal its one kill, which releases it.
    pub(crate) fn end_terminal(&mut self, id: TerminalId, err: Option<String>) {
        let core = &mut *self.core;
        // Removed first, so `sync_terminals` sends no second kill; this one sends no signal, it
        // only has the service release the PTY and input thread.
        let Some(buffer) = core.terminals.remove(&id) else {
            return;
        };
        self.effects.push(Effect::Terminal(TerminalOp::Kill { id }));
        if core.buffers.get(buffer).is_some() {
            core.kill_buffer(buffer);
            if let Some(err) = err {
                core.echo(err);
            }
        }
    }
}

/// Whether `key` stays with the editor in char mode, as #89 and #233 decided.
pub(crate) fn editor_key(key: Key) -> bool {
    let ctrl = |c| {
        key == Key {
            ctrl: true,
            ..Key::plain(KeyCode::Char(c))
        }
    };
    let meta_x = Key {
        meta: true,
        ..Key::plain(KeyCode::Char('x'))
    };
    // C-S-v pastes on Linux, as in its terminals, where Super belongs to the desktop.
    let paste = !cfg!(target_os = "macos")
        && key
            == Key {
                ctrl: true,
                shift: true,
                ..Key::plain(KeyCode::Char('v'))
            };
    // Text scale, as in Linux terminals.
    let zoom = !cfg!(target_os = "macos") && ['=', '+', '-', '0'].into_iter().any(ctrl);
    key.super_ || ctrl('x') || ctrl('c') || key == meta_x || paste || zoom
}

impl Core {
    /// The current buffer's terminal while it takes keys: its shell is live, it isn't in copy
    /// mode and no key sequence is pending.
    pub(crate) fn char_mode(&self) -> Option<TerminalId> {
        let buffer = self.windows.window.buffer();
        (self.pending.is_empty() && !self.current().copy_mode())
            .then(|| self.terminals.iter().find(|&(_, &b)| b == buffer))
            .flatten()
            .map(|(&id, _)| id)
    }

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
            .layout(frame, (cell_width, line), self.settings.mode_line_height())
            .into_iter())
        .map(|(id, rect)| {
            let window = match id == self.windows.selected {
                true => &self.windows.window,
                false => &self.windows.others[&id],
            };
            let rows = self.settings.text_rows(rect.height) as u16;
            let cols = (((rect.width - 2. * TEXT_INSET) / cell_width) as u16).max(1);
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
    use crate::settings::Settings;

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
    fn prompt_marks_keep_their_lines() {
        let mut t = fed(
            3,
            10,
            "\x1b]133;A\x07$ \x1b]133;B\x07ls\r\n\x1b]133;C\x07out\r\n\x1b]133;D;2\x07",
        );
        use Mark::*;
        let marks = [
            (0, 0, Prompt),
            (0, 2, Input),
            (1, 0, Output),
            (2, 0, End(Some(2))),
        ];
        assert_eq!(t.marks(), marks);
        t.feed(b"\r\n\r\n\r\n\x1b]13");
        t.feed(b"3;D\x1b\\");
        assert_eq!(t.history(), 3);
        assert_eq!(t.marks()[..4], marks);
        assert_eq!(t.marks()[4], (5, 0, End(None)));
    }

    #[test]
    fn marks_on_the_alternate_screen_are_ignored() {
        let t = fed(3, 10, "\x1b[?1049h\x1b]133;A\x07\x1b[?1049l");
        assert_eq!(t.marks(), []);
    }

    #[test]
    fn clearing_the_scrollback_drops_the_marks() {
        let mut t = fed(
            3,
            10,
            "\x1b]133;A\x07\r\n\x1b[H\x1b[2J\x1b[3J\x1b]133;B\x07",
        );
        assert_eq!(t.marks(), [(0, 0, Mark::Input)]);
        t.feed(b"\x1b[?1049h\x1b[3J\x1b[?1049l");
        assert_eq!(t.marks(), [(0, 0, Mark::Input)]);
        t.feed(b"\x1bc");
        assert_eq!(t.marks(), []);
    }

    #[test]
    fn a_full_scrollback_stops_the_marks() {
        let mut t = fed(3, 10, "\x1b]133;A\x07");
        t.feed(&b"x\r\n".repeat(SCROLLBACK + 3));
        assert_eq!(t.history(), SCROLLBACK);
        t.feed(b"\x1b]133;A\x07");
        assert_eq!(t.marks(), []);
        t.feed(b"\x1b[3J\x1b]133;A\x07");
        assert_eq!(t.marks(), [(2, 0, Mark::Prompt)]);
    }

    #[test]
    fn the_running_command_and_last_failure_follow_the_marks() {
        let state = |t: &Terminal| (t.running().map(String::from), t.failed());
        let run = |t: &mut Terminal, end: &str| {
            t.feed(b"\x1b]133;C;cmdline_url=cargo%20test\x07");
            assert_eq!(state(t), (Some("cargo test".into()), None));
            t.feed(end.as_bytes());
        };
        let mut t = Terminal::new(3, 20);
        run(&mut t, "\x1b]133;D;101\x07");
        assert_eq!(state(&t), (None, Some(101)));
        t.feed(b"\x1b]133;A\x07\x1b]133;B\x07");
        assert_eq!(state(&t), (None, Some(101)), "an empty prompt keeps it");
        run(&mut t, "\x1b]133;D;0\x07");
        assert_eq!(state(&t), (None, None));
        run(&mut t, "\x1b]133;A\x07");
        assert_eq!(state(&t), (None, None), "a shell that sends no end");
        t.feed(b"\x1b]133;C\x07\x1b]133;C;cmdline_url=%zz\x07");
        assert_eq!(state(&t), (None, None), "no text to show");
    }

    #[test]
    fn the_running_command_outlives_the_marks() {
        let mut t = Terminal::new(2, 20);
        t.feed("\x1b]133;C;cmdline_url=vim\x07\x1b[?1049h".as_bytes());
        t.feed(b"\x1b]133;D;1\x07");
        assert_eq!(
            (t.running(), t.failed()),
            (None, Some(1)),
            "on the alternate screen"
        );
        t.feed(b"\x1b[?1049l");
        t.feed("\n".repeat(SCROLLBACK + 2).as_bytes());
        t.feed("\x1b]133;C;cmdline_url=make\x07".as_bytes());
        assert_eq!(t.marks(), []);
        assert_eq!(t.running(), Some("make"), "past a full scrollback");
        t.feed(b"\x1b]133;D;2\x07");
        assert_eq!((t.running(), t.failed()), (None, Some(2)));
    }

    #[test]
    fn osc_7_sets_a_local_cwd() {
        let mut t = fed(3, 10, "\x1b]7;file://localhost/tmp/a%20b\x07");
        assert_eq!(t.cwd(), Some(Path::new("/tmp/a b")));
        t.feed(b"\x1b]7;file:///home\x1b\\");
        assert_eq!(t.cwd(), Some(Path::new("/home")));
        let host = String::from_utf8_lossy(&hostname()).into_owned();
        t.feed(format!("\x1b]7;kitty-shell-cwd://{host}/srv\x07").as_bytes());
        assert_eq!(t.cwd(), Some(Path::new("/srv")));
        for url in [
            "file://elsewhere.example/etc",
            "file://localhost",
            "file:tmp",
            "http://localhost/x",
            "file:///bad%zz",
            "file:///bad%+f",
            "file:///nul%00",
        ] {
            t.feed(format!("\x1b]7;{url}\x07").as_bytes());
            assert_eq!(t.cwd(), Some(Path::new("/srv")), "{url}");
        }
    }

    impl Terminal {
        /// Copy mode's text and cursor, without its colours.
        fn copy(&self) -> (String, usize, usize) {
            let (text, _, line, col, _) = self.copy_text(&Settings::default().theme);
            (text, line, col)
        }
    }

    #[test]
    fn copy_text_is_the_scrollback_then_the_screen() {
        let t = fed(3, 10, "1\r\n2\r\n3\r\n4\r\n$ ");
        assert_eq!(t.copy(), ("1\n2\n3\n4\n$".into(), 4, 2));
        // Blank rows below the cursor go; the cursor's own row stays.
        assert_eq!(fed(4, 10, "a\r\n").copy(), ("a\n".into(), 1, 0));
        let t = fed(2, 4, "abcdefghij\r\nx");
        assert_eq!(
            t.copy(),
            ("abcdefghij\nx".into(), 1, 1),
            "wrapped rows join"
        );
        let t = fed(2, 4, "abcdef");
        assert_eq!(
            t.copy(),
            ("abcdef".into(), 0, 6),
            "the cursor on a continuation row"
        );
        let t = fed(2, 10, "1\r\n2\r\n3\x1b[?1049h\x1b[Hvim");
        assert_eq!(t.copy(), ("vim".into(), 0, 3), "the alternate screen alone");
    }

    #[test]
    fn copy_text_keeps_colours() {
        let theme = Settings::default().theme;
        let t = fed(3, 4, "a\x1b[31mbcdef\x1b[0m g\r\n\x1b[44m  \x1b[0m\r\n");
        let (text, faces, ..) = t.copy_text(&theme);
        assert_eq!(text, "abcdef g\n\n");
        let red = Face {
            fg: Some(theme.ansi[1]),
            ..Face::default()
        };
        // A run split by the wrap is two faces; the blank blue row has no text to colour.
        assert_eq!(faces, [(1..4, red), (4..6, red)]);
        let t = fed(1, 10, "\x1b[1mb\x1b[0;3mi\x1b[0;4mu\x1b[0m.");
        let (text, faces, ..) = t.copy_text(&theme);
        let fg = Some(theme.foreground());
        let attrs: Vec<_> = (faces.iter())
            .inspect(|(_, face)| assert_eq!(face.fg, fg))
            .map(|(r, f)| (r.clone(), f.bold, f.italic, f.underline))
            .collect();
        assert_eq!(text, "biu.");
        let want = [(0..1, true, false, false), (1..2, false, true, false)];
        assert_eq!(attrs, [&want[..], &[(2..3, false, false, true)]].concat());
    }

    #[test]
    fn wide_chars_take_one_char() {
        assert_eq!(fed(1, 10, "日本x").screen(), "日本x");
    }

    /// Each run of the display's row `row` as (col, cells, text, fg, bg).
    fn runs(t: &Terminal, row: usize) -> Vec<(u16, u16, String, u32, Option<u32>)> {
        let theme = Settings::default().theme;
        let rows = t.display(&theme).rows;
        (rows[row].iter())
            .map(|r| (r.col, r.cells, r.text.clone(), r.fg, r.bg))
            .collect()
    }

    #[test]
    fn runs_sit_at_their_columns() {
        let fg = Settings::default().theme.foreground();
        let t = fed(3, 10, "ab\x1b[1;6Hc");
        let want = vec![(0, 2, "ab".into(), fg, None), (5, 1, "c".into(), fg, None)];
        assert_eq!(runs(&t, 0), want);
        assert_eq!(runs(&t, 1), vec![]);
        let t = fed(3, 10, "1\r\n2\r\n3\r\n4\r\n5");
        let rows = t.display(&Settings::default().theme).rows;
        let text: Vec<_> = rows.iter().map(|r| r[0].text.as_str()).collect();
        assert_eq!(text, ["3", "4", "5"]);
    }

    #[test]
    fn colors_resolve_against_the_theme() {
        let theme = Settings::default().theme;
        assert_eq!(runs(&fed(1, 10, "\x1b[31mx"), 0)[0].3, theme.ansi[1]);
        assert_eq!(runs(&fed(1, 10, "\x1b[91mx"), 0)[0].3, theme.ansi[9]);
        let t = fed(
            1,
            10,
            "\x1b[38;5;196ma\x1b[38;5;244mb\x1b[38;2;1;2;3mc\x1b[38;5;16md",
        );
        let fgs: Vec<_> = runs(&t, 0).iter().map(|r| r.3).collect();
        assert_eq!(fgs, [0xff0000, 0x808080, 0x010203, 0x000000]);
        let t = fed(1, 10, "\x1b[48;5;21m \x1b[7mx");
        assert_eq!(
            runs(&t, 0),
            vec![
                (0, 1, " ".into(), theme.foreground(), Some(0x0000ff)),
                (1, 1, "x".into(), 0x0000ff, Some(theme.foreground())),
            ]
        );
        let t = fed(1, 10, "\x1b[7mx");
        let inverse = (theme.background(), Some(theme.foreground()));
        assert_eq!((runs(&t, 0)[0].3, runs(&t, 0)[0].4), inverse);
        assert_eq!(
            runs(&fed(1, 10, "\x1b[2;38;2;150;90;30mx"), 0)[0].3,
            0x643c14
        );
        let t = fed(1, 10, "\x1b]4;1;rgb:12/34/56\x07\x1b[31mx");
        assert_eq!(runs(&t, 0)[0].3, 0x123456, "OSC 4 wins over the theme");
    }

    #[test]
    fn attributes_split_runs() {
        // A curly underline counts as an underline, so `c` and `d` share a run.
        let t = fed(1, 10, "\x1b[1ma\x1b[0;3mb\x1b[0;4mc\x1b[0;4:3md");
        let d = t.display(&Settings::default().theme);
        let flags: Vec<_> = (d.rows[0].iter())
            .map(|r| (r.text.as_str(), r.bold, r.italic, r.underline))
            .collect();
        assert_eq!(
            flags,
            [
                ("a", true, false, false),
                ("b", false, true, false),
                ("cd", false, false, true),
            ]
        );
    }

    #[test]
    fn the_cursor_follows_the_program() {
        let theme = Settings::default().theme;
        let cursor =
            |t: &Terminal| (t.display(&theme).cursor).map(|c| (c.row, c.col, c.cells, c.shape));
        let mut t = fed(2, 10, "ab");
        assert_eq!(cursor(&t), Some((0, 2, 1, CursorShape::Block)));
        t.feed(b"\x1b[6 q");
        assert_eq!(cursor(&t), Some((0, 2, 1, CursorShape::Beam)));
        t.feed(b"\x1b[4 q\r\n");
        assert_eq!(cursor(&t), Some((1, 0, 1, CursorShape::Underline)));
        t.feed(b"\x1b[?25l");
        assert_eq!(cursor(&t), None);
        t.feed(b"\x1b[?25h\x1b[2 q\x1b[1;1H");
        assert_eq!(cursor(&t), Some((0, 0, 1, CursorShape::Block)));
        t.feed("日\x1b[1;1H".as_bytes());
        assert_eq!(cursor(&t), Some((0, 0, 2, CursorShape::Block)));
    }

    #[test]
    fn wide_chars_take_two_cells() {
        let t = fed(2, 10, "日本x\r\nabcde");
        let cols: Vec<_> = runs(&t, 0)
            .iter()
            .map(|r| (r.0, r.1, r.2.clone()))
            .collect();
        assert_eq!(
            cols,
            [(0, 2, "日".into()), (2, 2, "本".into()), (4, 1, "x".into())]
        );
        assert_eq!(runs(&t, 1)[0].2, "abcde", "e at column 4 too");
        let t = fed(2, 10, "🦀x\r\nabc");
        let row: Vec<_> = runs(&t, 0).iter().map(|r| (r.0, r.1)).collect();
        assert_eq!(row, [(0, 2), (2, 1)], "x at column 2, as c below");
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

    #[test]
    fn paste_is_bracketed_when_asked() {
        let mut t = Terminal::new(3, 10);
        assert_eq!(t.paste("a\nb\x1b[201~c"), b"a\nb\x1b[201~c");
        t.feed(b"\x1b[?2004h");
        assert_eq!(t.paste("a\nb\x1b[201~c"), b"\x1b[200~a\nb[201~c\x1b[201~");
    }

    #[test]
    fn keys_encode_as_xterm() {
        let mut t = Terminal::new(2, 10);
        let enc = |t: &Terminal, keys: &str| -> Vec<u8> {
            (crate::key::parse_keys(keys).unwrap().into_iter())
                .flat_map(|k| t.encode(k).unwrap_or_else(|| panic!("{k} has no encoding")))
                .collect()
        };
        for (keys, want) in [
            ("hé SPC", "hé "),
            (
                "C-a C-e C-r C-g C-u C-S-a C-SPC C-@ C-[ C-\\ C-] C-^ C-_ C-S-_ C-?",
                "\x01\x05\x12\x07\x15\x01\0\0\x1b\x1c\x1d\x1e\x1f\x1f\x7f",
            ),
            (
                "M-b M-S-b C-M-a M-RET M-DEL",
                "\x1bb\x1bB\x1b\x01\x1b\r\x1b\x7f",
            ),
            (
                "RET S-RET TAB S-TAB ESC DEL C-DEL S-DEL",
                "\r\n\t\x1b[Z\x1b\x7f\x08\x7f",
            ),
            (
                "<prior> <next> <delete> C-<delete>",
                "\x1b[5~\x1b[6~\x1b[3~\x1b[3;5~",
            ),
            (
                "<f1> <f4> <f5> <f10> <f11> <f12> S-<f1> C-<f5>",
                "\x1bOP\x1bOS\x1b[15~\x1b[21~\x1b[23~\x1b[24~\x1b[1;2P\x1b[15;5~",
            ),
        ] {
            assert_eq!(String::from_utf8(enc(&t, keys)).unwrap(), want, "{keys}");
        }
        let arrows =
            "<up> <down> <right> <left> <home> <end> C-<left> M-<up> S-<end> C-M-S-<right>";
        let modified = "\x1b[1;5D\x1b[1;3A\x1b[1;2F\x1b[1;8C";
        let normal = format!("\x1b[A\x1b[B\x1b[C\x1b[D\x1b[H\x1b[F{modified}");
        assert_eq!(String::from_utf8(enc(&t, arrows)).unwrap(), normal);
        t.feed(b"\x1b[?1h");
        let app = format!("\x1bOA\x1bOB\x1bOC\x1bOD\x1bOH\x1bOF{modified}");
        assert_eq!(String::from_utf8(enc(&t, arrows)).unwrap(), app);
        t.feed(b"\x1b[?1l");
        assert_eq!(enc(&t, "<up>"), b"\x1b[A");
        let ctrl_1 = crate::key::parse_keys("C-1").unwrap()[0];
        assert_eq!(t.encode(ctrl_1), None);
        assert_eq!(t.encode(Key::plain(KeyCode::F(0))), None);
    }

    mod in_the_core {
        use crate::buffer::Decoration;
        use crate::harness::{Harness, mark};
        use crate::settings::FaceName;
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
            for command in [
                "forward-char",
                "next-line",
                "end-of-buffer",
                "mark-whole-buffer",
            ] {
                h.keys(&format!("M-x {command} RET"));
            }
            assert_eq!((h.point(), h.text()), (0, String::new()));
            h.keys("M-x newline RET");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            let terminal = h.core.buffers.get(buffer).unwrap().terminal().unwrap();
            assert_eq!(terminal.screen(), "shown\n");
        }

        /// A harness showing a live terminal, collecting effects.
        fn shown() -> (Harness, TerminalId) {
            let mut h = Harness::new(&[]);
            let (id, buffer) = h.core.create_terminal("t", 2, 10);
            h.core.show_buffer(buffer);
            h.collect_effects = true;
            (h, id)
        }

        /// What `keys` wrote to the shell, joined.
        fn typed(h: &mut Harness, keys: &str) -> String {
            h.keys(keys);
            let bytes: Vec<u8> = writes(h).into_iter().flat_map(|(_, b)| b).collect();
            String::from_utf8(bytes).unwrap()
        }

        #[test]
        fn char_mode_sends_keys_to_the_shell() {
            let (mut h, id) = shown();
            h.keys("echo SPC hi RET");
            let bytes: Vec<u8> = (writes(&mut h).into_iter())
                .inspect(|(i, _)| assert_eq!(*i, id))
                .flat_map(|(_, b)| b)
                .collect();
            assert_eq!(bytes, b"echo hi\r");
            let readline = typed(&mut h, "C-a C-e C-r M-b C-g C-u C-y");
            assert_eq!(readline, "\x01\x05\x12\x1bb\x07\x15\x19");
            assert_eq!(h.core.message(), None, "no command ran");
            assert_eq!(typed(&mut h, "C-c C-c"), "\x03");
            h.send(Msg::Insert {
                text: "日é".into(),
                replace: Some(0..1),
            });
            assert_eq!(writes(&mut h), [(id, "日é".as_bytes().to_vec())]);
        }

        #[test]
        fn arrows_follow_the_cursor_mode() {
            let (mut h, id) = shown();
            assert_eq!(typed(&mut h, "<up> C-<left>"), "\x1b[A\x1b[1;5D");
            output(&mut h, id, "\x1b[?1h");
            assert_eq!(typed(&mut h, "<up> C-<left>"), "\x1bOA\x1b[1;5D");
        }

        #[test]
        fn editor_keys_stay_with_the_editor() {
            let (mut h, _) = shown();
            assert_eq!(typed(&mut h, "M-x vterm"), "");
            h.send(Msg::Insert {
                text: "!".into(),
                replace: None,
            });
            assert!(writes(&mut h).is_empty());
            assert_eq!(h.text(), "vterm!", "typed into the minibuffer");
            h.keys("C-g");
            assert_eq!(typed(&mut h, "C-x b *scratch*<main> RET"), "");
            assert_eq!(h.core.current().name, "*scratch*<main>");
            h.keys("C-x b RET");
            assert_eq!(h.core.current().name, "t");
            // Under a prefix, C-g quits it rather than reaching the shell.
            assert_eq!(typed(&mut h, "C-x C-g"), "");
            assert_eq!(h.core.message(), Some("Quit"));
            assert_eq!(typed(&mut h, "s-z C-c a"), "");
            assert_eq!(h.core.message(), Some("C-c a is undefined"));
        }

        /// A shown live terminal holding `seq 1 5000`'s output and a `$ ` prompt, in copy mode.
        fn copying() -> (Harness, TerminalId, String) {
            let (mut h, id) = shown();
            let seq: String = (1..=5000).map(|n| format!("{n}\r\n")).collect();
            output(&mut h, id, &(seq + "$ "));
            assert_eq!(typed(&mut h, "C-c C-t"), "");
            let text = (1..=5000).map(|n| format!("{n}\n")).collect::<String>() + "$";
            (h, id, text)
        }

        /// The clipboard text set since the last drain of effects, if any.
        fn clipboard(h: &Harness) -> Option<String> {
            (h.effects.iter().rev()).find_map(|e| match e {
                Effect::SetClipboard(text) => Some(text.clone()),
                _ => None,
            })
        }

        fn line(h: &Harness) -> usize {
            h.core.current().line_col(h.point()).unwrap().0
        }

        #[test]
        fn copy_mode_shows_the_scrollback_as_text() {
            let (mut h, _, text) = copying();
            assert_eq!((h.text(), h.point()), (text.clone(), text.len()));
            let frame = h.frame(800., 300.);
            let display = &frame.windows[0].display;
            assert!(display.terminal.is_none() && display.cursor.is_some());
            let hl = Decoration::Background(FaceName::HlLine);
            assert_eq!(display.lines.last().unwrap().decorations, [hl]);
            assert_eq!(display.mode_line.text(), "t  copy │ 5001:1");
            assert_eq!(display.mode_line.left[0][0].1, FaceName::ModeLineBufferId);
            assert_eq!(display.mode_line.right[0][0].1, FaceName::StatusInfo);
            // Motion, swiper and the mark work; inserting fails; the shell hears nothing.
            h.keys("M-< C-n C-SPC C-e M-w");
            assert_eq!(clipboard(&h).as_deref(), Some("2"));
            assert!(writes(&mut h).is_empty());
            assert_eq!(typed(&mut h, "C-s 4999 RET"), "");
            assert_eq!(line(&h), 4998);
            assert_eq!(typed(&mut h, "x"), "");
            assert_eq!(h.core.message(), Some("Buffer is read-only"));
            assert_eq!(h.text(), text);
        }

        #[test]
        fn a_wrapped_command_is_one_line() {
            let (mut h, id) = shown();
            output(&mut h, id, "$ echo 0123456789abc\r\n0123456789abc\r\n$ ");
            h.keys("C-c C-t");
            assert_eq!(h.text(), "$ echo 0123456789abc\n0123456789abc\n$");
        }

        /// Three commands, each between OSC 133 marks, then a fourth prompt.
        const COMMANDS: &str = "\x1b]133;A\x07$ \x1b]133;B\x07echo 1\r\n\x1b]133;C\x071\r\n\x1b]133;D;0\x07\
            \x1b]133;A\x07$ \x1b]133;B\x07echo 2\r\n\x1b]133;C\x072\r\n\x1b]133;D;0\x07\
            \x1b]133;A\x07$ \x1b]133;B\x07echo 3\r\n\x1b]133;C\x073\r\n\x1b]133;D;0\x07\
            \x1b]133;A\x07$ \x1b]133;B\x07";

        #[test]
        fn prompt_jumps_land_on_each_commands_input() {
            let (mut h, id) = shown();
            output(&mut h, id, COMMANDS);
            h.keys("C-c C-t");
            assert_eq!(h.text(), "$ echo 1\n1\n$ echo 2\n2\n$ echo 3\n3\n$");
            let input = |line| h.core.current().char_idx(line, 2).unwrap();
            let [first, second, third] = [input(0), input(2), input(4)];
            h.keys("C-c C-p");
            assert_eq!(h.point(), third);
            h.keys("C-c C-p");
            assert_eq!(h.point(), second);
            h.keys("C-c C-p");
            assert_eq!(h.point(), first);
            h.keys("C-c C-p");
            assert_eq!(h.core.message(), Some("No prompt found"));
            assert_eq!(h.point(), first);
            h.keys("C-c C-n");
            assert_eq!(h.point(), second);
            h.keys("C-u - C-c C-n");
            assert_eq!(h.point(), first, "a negative count goes back");
            h.keys("C-u 9 C-c C-n");
            assert_eq!(h.point(), h.text().len(), "stops at the newest prompt");
            h.keys("C-u 2 C-c C-p");
            assert_eq!(h.point(), second, "skips one prompt");
        }

        #[test]
        fn a_prompt_without_input_mark_is_its_start() {
            let (mut h, id) = shown();
            output(&mut h, id, "\x1b]133;A\x07$ ls\r\nx\r\n$ ");
            h.keys("C-c C-t C-c C-p");
            assert_eq!(h.point(), 0);
        }

        #[test]
        fn a_prompt_jump_finds_a_wrapped_input() {
            let (mut h, id) = shown();
            // The 11-char prompt wraps in 10 columns, so the input starts on the second row.
            output(
                &mut h,
                id,
                "\x1b]133;A\x07~/long/dir$\x1b]133;B\x07 ls\r\n\x1b]133;A\x07$ \x1b]133;B\x07",
            );
            h.keys("C-c C-t C-c C-p");
            let text = h.text();
            assert_eq!(&text[..h.point()], "~/long/dir$");
        }

        #[test]
        fn no_marks_no_prompt() {
            let (mut h, _, text) = copying();
            h.keys("C-c C-p");
            assert_eq!(h.core.message(), Some("No prompt found"));
            assert_eq!(h.point(), text.len());
        }

        #[test]
        fn marking_output_from_it_or_its_command_line_copies_it() {
            let (mut h, id) = shown();
            output(
                &mut h,
                id,
                "\x1b]133;A\x07$ \x1b]133;B\x07t\r\n\x1b]133;C\x07a\r\nb\r\n\x1b]133;D;0\x07\
                 \x1b]133;A\x07$ \x1b]133;B\x07",
            );
            h.keys("C-c C-t");
            assert_eq!(h.text(), "$ t\na\nb\n$");
            h.keys("C-c C-p C-c C-p C-n C-n C-c C-r");
            assert_eq!((h.point(), mark(&h)), (4, (Some(7), true)));
            h.keys("RET");
            assert_eq!(h.core.kills.current_kill(0).unwrap(), "a\nb");
            h.keys("C-c C-t C-c C-p C-c C-p C-c C-r RET");
            assert_eq!(h.core.kills.current_kill(0).unwrap(), "a\nb");
        }

        #[test]
        fn output_without_an_end_runs_to_the_next_prompt_or_the_text_end() {
            let (mut h, id) = shown();
            output(
                &mut h,
                id,
                "\x1b]133;A\x07$ \x1b]133;B\x07t\r\n\x1b]133;C\x07a\r\n\
                 \x1b]133;A\x07$ \x1b]133;B\x07u\r\n\x1b]133;C\x07b\r\nc",
            );
            h.keys("C-c C-t");
            assert_eq!(h.text(), "$ t\na\n$ u\nb\nc");
            h.keys("C-c C-r RET");
            assert_eq!(h.core.kills.current_kill(0).unwrap(), "b\nc");
            h.keys("C-c C-t M-< C-c C-r RET");
            assert_eq!(h.core.kills.current_kill(0).unwrap(), "a");
        }

        #[test]
        fn no_marks_no_command() {
            let (mut h, _, text) = copying();
            h.keys("C-c C-r");
            assert_eq!(h.core.message(), Some("No command at point"));
            assert_eq!(h.point(), text.len());
        }

        #[test]
        fn copy_mode_draws_the_colours_until_it_ends() {
            let (mut h, id) = shown();
            output(&mut h, id, "\x1b[31mred\x1b[0m\r\n$ ");
            h.keys("C-c C-t");
            let red = h.core.settings().theme.ansi[1];
            let frame = h.frame(800., 300.);
            let span = &frame.windows[0].display.lines[0].spans[0];
            assert_eq!((span.text.as_str(), span.face.fg), ("red", Some(red)));
            h.keys("C-c C-t");
            assert_eq!(h.core.current().properties().count(), 0);
            let frame = h.frame(800., 300.);
            let lines = &frame.windows[0].display.lines;
            assert!(lines.iter().all(|l| l.decorations.is_empty()), "char mode");
        }

        #[test]
        fn copy_mode_on_the_alternate_screen_is_the_screen() {
            let (mut h, id) = shown();
            output(&mut h, id, "1\r\n2\r\n3\x1b[?1049h\x1b[Hvim");
            h.keys("C-c C-t");
            assert_eq!((h.text(), h.point()), ("vim".into(), 3));
        }

        /// The mode line of `t`, shown as `shown()` leaves it, after `bytes`.
        fn mode_line(h: &mut Harness, id: TerminalId, bytes: &str) -> crate::display::ModeLine {
            output(h, id, bytes);
            h.frame(800., 300.).windows.remove(0).display.mode_line
        }

        #[test]
        fn the_mode_line_shows_each_state_of_a_terminal() {
            let (mut h, id) = shown();
            let run = "\x1b]133;C;cmdline_url=cargo%20test\x07";
            let line = mode_line(&mut h, id, run);
            assert_eq!(line.text(), "t  ◉ cargo test │ char");
            assert_eq!(line.right[0][0].1, FaceName::StatusRunning);
            assert_eq!(line.right[1][0].1, FaceName::StatusWaiting);
            let line = mode_line(&mut h, id, "\x1b]133;D;101\x07");
            assert_eq!(line.text(), "t  exit 101 │ char");
            assert_eq!(line.right[0][0].1, FaceName::StatusFailed);
            h.keys("C-c C-t");
            let line = h.frame(800., 300.).windows.remove(0).display.mode_line;
            assert_eq!(line.text(), "t  exit 101 │ copy │ 1:0");
            assert_eq!(line.right[1][0].1, FaceName::StatusInfo);
            h.keys("C-c C-t");
            let again = "\x1b]133;C;cmdline_url=a%0Ab\x07";
            assert_eq!(mode_line(&mut h, id, again).text(), "t  ◉ a b │ char");
            let line = mode_line(&mut h, id, "\x1b]133;D;0\x07");
            assert_eq!(line.text(), "t  char", "a success shows nothing");
        }

        #[test]
        fn the_mode_line_shows_the_shells_directory() {
            let (mut h, id) = shown();
            let line = mode_line(&mut h, id, "\x1b]7;file:///srv\x07");
            assert_eq!(line.text(), "t │ /srv  char");
            assert_eq!(line.left[1][0].1, FaceName::ModeLineDim);
            let home = std::env::home_dir().unwrap();
            let cd = |path: &std::path::Path| format!("\x1b]7;file://{}\x07", path.display());
            assert_eq!(mode_line(&mut h, id, &cd(&home)).text(), "t │ ~  char");
            let line = mode_line(&mut h, id, &cd(&home.join("crates")));
            assert_eq!(line.text(), "t │ ~/crates  char");
        }

        #[test]
        fn a_narrow_mode_line_keeps_the_end_of_the_directory() {
            let (mut h, id) = shown();
            let line = mode_line(&mut h, id, "\x1b]7;file:///srv/one/two/three\x07");
            let left = |cols| {
                let theme = &h.core.settings().theme;
                let spans = line.draw(cols, true, theme).left;
                spans.iter().map(|s| s.text.as_str()).collect::<String>()
            };
            assert_eq!(left(28), "t │ /srv/one/two/three");
            assert_eq!(left(20), "t │ …two/three");
            assert_eq!(left(10), "t", "no separator is left behind");
        }

        #[test]
        fn output_waits_for_copy_mode_to_end() {
            let (mut h, id, text) = copying();
            h.keys("M-<");
            output(&mut h, id, "new\r\n");
            assert_eq!((h.text(), h.point()), (text, 0));
            assert_eq!(typed(&mut h, "C-c C-t"), "", "leaves without copying");
            assert_eq!((h.text(), h.point()), (String::new(), 0));
            assert!(h.core.kills.current_kill(0).is_err());
            let screen = h.core.current().terminal().unwrap().screen();
            assert_eq!(screen, "$ new\n");
            let frame = h.frame(800., 300.);
            assert!(frame.windows[0].display.terminal.is_some());
            assert_eq!(frame.windows[0].display.mode_line.text(), "t  char");
            let mode = &frame.windows[0].display.mode_line.right[0][0];
            assert_eq!(mode.1, FaceName::StatusWaiting);
            assert_eq!(typed(&mut h, "a"), "a", "char mode again");
        }

        #[test]
        fn ret_copies_the_region_or_the_line_and_leaves() {
            let (mut h, _, _) = copying();
            h.keys("M-< C-SPC C-f C-f RET");
            assert_eq!(clipboard(&h).as_deref(), Some("1\n"));
            assert!(writes(&mut h).is_empty());
            assert_eq!(h.core.kills.current_kill(0).unwrap(), "1\n");
            assert!(!h.core.current().copy_mode() && h.text().is_empty());
            assert!(h.frame(800., 300.).windows[0].display.terminal.is_some());
            assert_eq!(typed(&mut h, "RET"), "\r", "char mode again");
            h.keys("C-c C-t M-< C-n C-f RET");
            assert_eq!(
                clipboard(&h).as_deref(),
                Some("2"),
                "the line, without a region"
            );
        }

        #[test]
        fn the_zoom_keys_stay_with_the_editor() {
            let (mut h, _) = shown();
            let key = if cfg!(target_os = "macos") {
                "s-="
            } else {
                "C-="
            };
            assert_eq!(typed(&mut h, key), "");
            assert_eq!(h.core.settings().font_size, 15.);
        }

        #[test]
        fn the_paste_key_pastes_into_the_shell() {
            let (mut h, id) = shown();
            let paste = if cfg!(target_os = "macos") {
                "s-v"
            } else {
                "C-S-v"
            };
            assert_eq!(typed(&mut h, paste), "");
            assert_eq!(h.core.message(), Some("Kill ring is empty"));
            h.clipboard = Some("ls\n".into());
            assert_eq!(typed(&mut h, paste), "ls\n");
            output(&mut h, id, "\x1b[?2004h");
            assert_eq!(typed(&mut h, paste), "\x1b[200~ls\n\x1b[201~");
            assert_eq!(typed(&mut h, "C-v"), "\x16", "C-v still reaches the shell");
            // The other platform's paste key is not one here.
            let (other, sent) = if cfg!(target_os = "macos") {
                ("C-S-v", "\x16")
            } else {
                ("s-v", "")
            };
            assert_eq!(typed(&mut h, other), sent);
            assert_eq!(h.text(), "", "nothing inserted");
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
        fn a_terminal_buffers_directory_is_its_shells() {
            let mut h = work();
            let app = h.path("app");
            h.keys("M-x vterm RET");
            ops(&mut h);
            // Without OSC 7, the directory the shell started in.
            h.keys("C-x C-f");
            assert_eq!(h.text(), format!("{}/", app.display()));
            h.keys("C-g M-x vterm RET");
            assert_eq!(ops(&mut h), [format!("spawn 1 {} 24x80", app.display())]);
            h.send(Msg::TerminalOutput {
                id: TerminalId(1),
                bytes: "\x1b]7;file:///tmp\x07".into(),
            });
            h.keys("C-x C-f");
            assert_eq!(h.text(), "/tmp/");
            h.keys("C-g M-x vterm RET");
            assert_eq!(ops(&mut h), ["spawn 2 /tmp 24x80"]);
        }

        #[test]
        fn the_terminal_follows_its_smallest_window() {
            let mut h = work();
            let line = h.core.settings().line_height();
            // 80 cells between the insets, 24.5 lines over the mode line.
            let chrome = h.core.settings().mode_line_height();
            h.frame(664., 24.5 * line + chrome);
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
            // The chrome leaves 11 rows in each.
            h.keys("C-x 2");
            assert_eq!(ops(&mut h), ["resize 0 11x80"]);
            h.frame(344., 24.5 * line + chrome);
            assert_eq!(ops(&mut h), ["resize 0 11x40"]);
            h.frame(344., 24.5 * line + chrome);
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
            assert_eq!(ops(&mut h), ["kill 0"]);
            h.send(Msg::TerminalSpawned {
                id: TerminalId(1),
                result: Err("/x: not a directory".into()),
            });
            assert!(!exists(&h, "vterm2<work>"));
            assert_eq!(h.core.message(), Some("/x: not a directory"));
            assert_eq!(ops(&mut h), ["kill 1"]);
            assert_eq!(name(&h), "*scratch*<work>");
        }

        #[test]
        fn the_frame_shows_the_screen() {
            let mut h = work();
            h.keys("M-x vterm RET");
            h.send(Msg::TerminalOutput {
                id: TerminalId(0),
                bytes: b"$ ls\r\n".to_vec(),
            });
            let frame = h.frame(800., 300.);
            let terminal = frame.windows[0].display.terminal.as_ref().unwrap();
            let first: Vec<_> = terminal.rows[0]
                .iter()
                .map(|r| (r.col, r.text.as_str()))
                .collect();
            assert_eq!(first, [(0, "$"), (2, "ls")], "blanks are not drawn");
            assert!(terminal.rows[1].is_empty());
            let cursor = terminal.cursor.as_ref().map(|c| (c.row, c.col));
            assert_eq!(
                (cursor, frame.windows[0].display.cursor),
                (Some((1, 0)), None)
            );
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
