# Proposal

## Why

The frontend-rendering spec says what is on screen is exactly the display model, but the frontend reads the core piecemeal: `Editor::render` (`crates/aiditor/src/main.rs:486-589`) calls `core.minibuffer()` (506), `core.current()` for the window title (510-512), `core.hint()` (514) and `core.message()` (526), then `FrameElement::prepaint` calls `core.frame()` (671), which builds the minibuffer a second time (`crates/aiditor-core/src/lib.rs:2273`) and clones the message, both unused. Each minibuffer build ticks the matcher and re-runs `window_display` on the minibuffer window: idempotent, but wasted work every frame. The title rule (buffer name plus ` *` when modified) lives in the frontend, untested. On top of that, `main.rs` is 1439 lines doing six jobs (startup, the editor, IME input, the heartbeat, key translation, painting), which makes this and later frontend work hard to review.

## What Changes

- `FrameDisplay` (`crates/aiditor-core/src/display.rs:23-31`) gains `hint: Option<Hint>` and `title: String`. The title rule moves into `Core::frame`, with a core test.
- `render` calls `core.frame()` once and draws the echo area, the minibuffer, the hint and the window title from it, handing `frame.windows` to the element: `FrameElement { editor, windows }`.
- `FrameElement::prepaint` uses the windows it was given, unless the windows area's size changed: then it sends `Msg::Resize` and rebuilds them with `core.frame()` in the same frame, as today, so live resize does not lag.
- `Core::hint()` and `Core::minibuffer()` become `pub(crate)` (only the core's harness tests use them besides the frontend); the `aiditor` crate's tests switch to `core.frame().minibuffer`.
- `crates/aiditor/src/main.rs` is split, as a pure move in its own commit, into `main.rs` (startup, args, logging), `editor.rs` (`Editor`, effects, `Render`, mouse, debug stats and heartbeat), `input.rs` (`to_key`, `EntityInputHandler`, IME helpers) and `element.rs` (`FrameElement` and painting helpers). Tests move with the code they test.

No user-visible behavior changes.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. The change sets `skip_specs: true`. No requirement enumerates the frame display's contents: `display-model` specifies the windows and the message in separate requirements, `minibuffer` speaks of "the frame display's minibuffer", and `keymaps` says only that "the display" shows the hint, which `docs/editor.md` already places in the frame's echo area. Carrying the hint in the frame display makes the code match those words; it does not change what any requirement asks. The title is not specified anywhere and keeps its current behavior.

## Impact

- `crates/aiditor-core/src/display.rs`, `crates/aiditor-core/src/lib.rs` (`Core::frame`, visibility of `hint` and `minibuffer`), core harness test for the title.
- `crates/aiditor/src/main.rs` split into `main.rs`, `editor.rs`, `input.rs`, `element.rs`; render and prepaint rewired.
- No new dependencies; `aiditor-core` still depends on neither gpui nor tokio.
- Drawing is touched, so a manual check on macOS is required (see tasks).
