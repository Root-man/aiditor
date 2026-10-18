# Proposal

## Why

There is no way to quit from the keyboard, and closing the window drops unsaved edits without a word. Issue #48 adds Emacs's `C-x C-c`, which offers to save each modified file buffer first.

## What Changes

- New command `save-buffers-kill-terminal` (`C-x C-c`, also `s-q`): for each modified buffer visiting a file it asks `Save file <path>? `, completing over `Yes` then `No`, matched case-insensitively. If any was declined it asks `Modified buffers exist; exit anyway? `, completing over `No` then `Yes`; anything but `Yes` keeps the editor open. It quits once every save it started has succeeded; a failed save cancels the quit. With nothing modified it quits at once.
- Closing the window (title bar, window manager) runs the same command instead of closing.
- New `Effect::Quit`, which the app carries out by quitting, and `Msg::Quit`, which the app sends when the window asks to close.
- Minibuffer prompts become owned strings, so a prompt can name the file.

Not in this change: `s-w` and `s-h`, Emacs's other `save-some-buffers` answers (`!`, `.`, `q`, `d`), quitting from inside the minibuffer.

## Capabilities

### New Capabilities
- `quitting`: quitting the app without losing unsaved file buffers.

### Modified Capabilities

None.

## Impact

- `crates/aiditor-core`: `lib.rs` (the command, the quit state, `Msg::Quit`, `Effect::Quit`, owned prompts), `keymap.toml`, harness.
- `crates/aiditor`: `main.rs` runs `Effect::Quit` and routes window close to `Msg::Quit`.
- `docs/editor.md`: the decisions taken here.
- No new dependencies.
