# Design

## Context

See proposal.md for why. The constraints that shape the approach:

- The windows area's size is known only in `FrameElement::prepaint`, from the element's bounds. Today prepaint compares it with `Editor::reported`, sends `Msg::Resize` when it changed, and lays the windows out with the new size in the same frame. The echo area and hint are flex children drawn by `render`, so their height (and hence the windows area) is known only after layout.
- `Core::frame_size` is `(0, 0, 0)` until the first resize (`crates/aiditor-core/src/lib.rs:1761`). `Editor::reported` starts at its default, zero, too, so the first frame always takes the resize path.
- GPUI 0.2.2 (`~/Repos/zed/crates/gpui/src/element.rs`, `Drawable`): `render` returns the element tree; each element value is then driven once through `request_layout`, `prepaint` (which `mem::take`s the draw phase, so it runs once per element value) and `paint`, all with `&mut self`. An element can therefore own data it was constructed with and move it out in prepaint.
- `Core::minibuffer()` and `Core::hint()` are called by the core's harness tests (`crates/aiditor-core/src/harness.rs`, a private module of the core) and by `main.rs`. `Core::message()` is used widely by tests and stays public.

## Goals / Non-Goals

**Goals:**
- Without a resize, a frame builds the core's frame display exactly once.
- The frontend draws everything from `FrameDisplay`.
- `main.rs` holds startup code only.

**Non-Goals:**
- Any drawing change, soft wrap, IME marked text at prompts (`main.rs:528` shortcut), `bounds_for_range` precision (`main.rs:393` shortcut).
- Redesigning `Editor::handle`'s effect match (if `shared-effect-executor` lands first it is already short; otherwise it moves as is).
- A frame-build counter in the core: the once-per-frame property is shown by code review, since `core.frame()` is called from exactly two frontend places (render, and prepaint's resize branch).

## Decisions

**The element owns the windows.** `struct FrameElement { editor: Entity<Editor>, windows: Vec<FrameWindow> }`. In prepaint: if the size changed, send `Msg::Resize` and set `self.windows = editor.core.frame().windows`; then shape `mem::take(&mut self.windows)`. Alternative: keep the frame on `Editor` between render and prepaint (e.g. `Option<FrameDisplay>`); rejected, since it puts per-frame state on a long-lived struct and relies on ordering the element already guarantees. Alternative: a `RequestLayoutState` carrying the windows; no gain over a field.

**On resize the rest of the frame is not rebuilt.** The echo area, minibuffer and hint `render` drew came from the pre-resize frame display. That is what happens today (render calls `core.minibuffer()` before prepaint resizes), so the minibuffer's column count lags one frame on resize as it does now; `Msg::Resize` makes the core notify, and the next frame catches up. Behavior-neutral.

**The title rule moves to `Core::frame`.** `title = format!("{}{}", current().name, if modified { " *" } else { "" })`, computed from the same `current()` the frontend uses now. `render` calls `window.set_window_title(&frame.title)`. Note: while a prompt is open the minibuffer is the selected window, so the title today reads ` *Minibuf-1*`; this change keeps that and the test covers only a plain and a modified buffer (see Open Questions).

**`hint` and `minibuffer` become `pub(crate)`.** Their only callers outside the core are the frontend's render (replaced by the frame display) and `main.rs` tests (switched to `e.core.frame().minibuffer`, `e.core.frame().message` where convenient). The harness keeps calling them.

**Two commits, move first.** Commit 1 splits `main.rs` into `main.rs`, `editor.rs`, `input.rs`, `element.rs` with no code change beyond `mod` lines, `use`s and `pub(crate)` visibility. Commit 2 does the rewiring. This keeps the behavioral diff reviewable on its own.

Split by today's layout (line numbers at 8fa1ddd, re-checked):

| Today | Goes to |
|---|---|
| `33-292` `Editor`, `Debug`, `impl Editor` (effects, mouse, `font`, `painted_at`) | `editor.rs` (IME helpers `ime_point`, `to_buffer`, `buffer_range` to `input.rs`) |
| `294-405` `impl EntityInputHandler for Editor` | `input.rs` |
| `407-434` `spawn_heartbeat` | `editor.rs` |
| `437-447` `read_config` | `main.rs` |
| `448-484` `to_key` | `input.rs` |
| `486-589` `impl Render for Editor` | `editor.rs` |
| `591-862` `Painted`, `Row`, `FrameElement`, `to_bounds`, `text_run`, `echo_line`, `line_runs` | `element.rs` |
| `864-992` `USAGE`, `Args`, `parse_args`, `LogPipe`, `messages_layer`, `main` | `main.rs` |

Tests: `parses_flags_and_file`, `rejects_bad_arguments`, `warnings_reach_the_core_without_colour_and_info_does_not` and `parse` stay in `main.rs`; `sorts_command_keys_from_typing` and its `key` helper go to `input.rs`; the gpui tests with their `editor`, `sequences`, `keystroke`, `painted` helpers and `the_echo_line_is_one_string_with_point_in_it` go to `editor.rs`. Where a helper is needed by tests in two files, it lives in `editor.rs` tests as `pub(crate)` rather than being duplicated.

## Risks / Trade-offs

- [The element is prepainted without its windows, e.g. if GPUI ever reused an element value] → `Drawable` takes the phase on prepaint, so a value is prepainted once; the windows are moved out with `mem::take`, so a second prepaint would draw no windows rather than stale ones, visible at once in the manual check.
- [Visual regressions are caught only by hand] → the manual macOS check in tasks: live resize, typing, IME marked text, the minibuffer with completion, the hint.
- [`--debug` stats lose meaning] → `shape` stays timed in prepaint around shaping only; `render` now includes building the frame display, which is where that cost now lives. Acceptable; the `core` figure is unchanged.
- [The move commit breaks `cfg(test)` imports] → each file's `mod tests` uses `super::*` plus what it needs; `cargo test` after commit 1 alone must pass.

## Open Questions

- Should the title show the buffer the prompt was opened from rather than ` *Minibuf-1*` while a prompt is open? Out of scope here (behavior change); a follow-up can decide once the rule is in the core and tested.
