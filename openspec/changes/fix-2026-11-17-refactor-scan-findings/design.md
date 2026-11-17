# Design

## Context

See proposal.md for the findings. The state these fixes build on:

- `aiditor-core` is pure state; `aiditor-services` does IO and talks to the core with `Effect`s and `Msg`s; the frontend renders `core.frame()`.
- Session buffers hold thousands of properties. `Buffer::remove_properties` walks all of them; `paint_prompt` calls it whenever the buffer's tick changed, which is every keystroke and every streamed chunk.
- `Window` stores `(top, top_row)`. `Window::scroll` clamps `top_row` to the top line's rows; the session prompt path in `frame.rs` and `Window::shown` read it without that clamp.
- `files::write` with `WriteExpectation::Update(expected)` refuses when the disk hash is not `expected`. The core sends each save with the buffer's disk hash at the time, and has no in-flight guard.
- Grep hits carry the whole matched line (`Hit.text`); the core keeps it and repeats it in each hit's label; session search runs `said` on each hit in `Core::handle`.
- Language selection lives in `syntax::for_file` (extensions), a second match for Markdown fence names, and `display::file_mode_line` (faces).

## Goals / Non-Goals

**Goals:**
- Each bug fix has a test that fails before it.
- Each performance fix keeps the visible result and, where a benchmark covers the path, shows a lower median.
- #13 moves code without changing it; every existing test still passes.

**Non-Goals:**
- An interval index for properties (the `shortcut:` at `buffer.rs:339` stays).
- A confirm prompt for large files, like Emacs's `large-file-warning-threshold`; a fixed refusal is enough for now.
- Incremental file index or watcher changes (scan items left out of the table).

## Decisions

### D1 · Clamp `top_row` where it is read (#2)

`window_rows` and `Window::shown` index `row_starts(buffer, top)[top_row]`. Both use `top_row.min(starts.len() - 1)`. Clamping on read covers every path that leaves a stale row (prompt mode, a streamed edit shortening the line, a widened window) without tracking each one. Alternative: re-clamp in every setter: more places, and a missed one panics again.

### D2 · Skip the prompt clear when it holds no properties (#3)

`paint_prompt` asks `properties_near(prompt range)`, which bisects, and calls `remove_properties` only when it returns something inside the range. Most keystrokes in a prompt without chips or pastes then cost one bisect. Alternative: bisect inside `remove_properties` (the scan's fuller fix): it helps every caller but changes the index upkeep; keep it for when a measurement asks for it.

### D3 · `C-c C-k` mirrors `C-c C-y` (#5)

`agent_reject_once` first takes `ask_at_point()` when it is sendable: a question there is skipped, a permission request there gets its `reject_once` option. Otherwise it keeps today's rule: the oldest pending request. The two commands then agree on what point selects.

### D4 · 256 MiB visit limit in the service (#7)

`FileOp::Open` checks `checked_metadata(path)?.len()` before `fs::read`, as `preview` does with `PREVIEW_LIMIT`. Over `VISIT_LIMIT = 256 MiB` it fails with `<path>: file too large (<n> MiB)`, shown as other read errors are. 256 MiB is far above any source or log file the user edits and well below what a rope plus undo can hold. The command-line visit uses the same read, so it refuses too, as an unreadable file does today.

### D5 · Unchanged bytes count as written (#12)

In the `Update` arm, when `on_disk == Some(hash(bytes))` the write returns `Ok(hash)` without writing. A second save of the same text after the first landed then succeeds. A second save of *different* text (edits between the two saves) still conflicts; holding the second save in the core until the first reply is a larger change, left out.

### D6 · Session-line reduction in the grep worker (#4)

`said` and `SAID_CHARS` move from `aiditor-core/src/grep.rs` to the services grep, which applies them when the search has session `names`. `Hit.text` then arrives cut. The core keeps showing `Hit.text` as it does for project grep. This moves pure code into services; it stays pure, and its tests move with it.

### D7 · One language table (#10)

One `fn language(name: &str) -> Option<(&'static str, &'static Grammar)>` in `syntax.rs` holds every extension and full name (`rust`, `rs`, `javascript`, `js`, `jsx`, `typescript`, `ts`, `tsx`, `elixir`, `ex`, `exs`, `heex`, `yaml`, `yml`, `toml`). `for_file` and the fence lookup call it; the second match goes. Markdown stays out of fences: a nested Markdown injection is a separate question. `file_mode_line`'s face match stays: it keys on the language name, which the table returns.

### D8 · Visual-row layout for wrapped windows (#14)

`display::build` counts rows as each line finishes: `max(1, ceil(chars / width))` for wrapped text, 1 for aligned and table lines. It stops when the count reaches `rows`. Under word wrap this is a lower bound, so the build may lay out a line more than needed but never fewer; `truncate(rows)` stays.

### D9 · `agents.rs` split (#13)

Follow the existing `agents/*.rs` pattern: `impl Session` / `impl Core` blocks with `use super::*`. Order: tests first (one move, 7,000 lines), then `asks.rs`, `tools.rs`, `config.rs`, one commit each so a reviewer diffs moves only. Lifecycle, `agent_update` and streaming stay in `agents.rs`.

### D10 · Small fixes, no decision needed

- #1: hoist `text.chars().count()`.
- #6: `Some(&["No", "Yes"])`.
- #8: `ProjectGrep::label` drops `:{hit.text}`.
- #9: open the prompt with `Some(&[])` and pass `files` to `add_candidates`.
- #11: `window.request_animation_frame()` after `Msg::Resize` in `prepaint`.
- #15: the core puts `0..number_len` with `GrepLineNumber` into `mb.syntax[i]`; `Minibuffer::numbered` and its frontend arm go.
- #16: guard `Cache::edit` calls with `grammar.locals.is_some()`.
- #17: `Core::log` deletes the first lines past 1,000, without undo, as appending does.
- #18: `text.get(..start).map_or(0, |s| s.chars().count())`.
- #19: `prepare` sets point on every window of the workspace showing the buffer when the request has a position.
- #20: `take_place` rewrites the pin's session id to the fork's and emits `WritePins`.

## Risks / Trade-offs

- [#11 depends on GPUI 0.2.2 dropping `notify` during prepaint] → verify in a running window before and after; if the frame already re-renders, drop the change.
- [#4 moves code across crates] → move `said`'s tests with it; session-search specs already pin the visible result.
- [#13 is a big diff] → moves only, one commit per file, no edits in the same commits.
- [#14 lower-bound row count under word wrap] → existing wrap tests plus a new paragraph case check that no row goes missing.
- [#7 a user with a 300 MiB log can't open it] → the echo says why; a confirm prompt can come later.
