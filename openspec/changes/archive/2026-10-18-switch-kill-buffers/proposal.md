# Proposal

## Why

Visiting a file leaves the old buffer unreachable: there is no way back to it and no way to get rid of it. Issue #42 adds Emacs's buffer switching and killing: `C-x b` with completion over buffers, most recent first, `C-x 4 b` and `C-x k`. Scoping the list to the workspace comes in M1.

## What Changes

- New command `switch-to-buffer` (`C-x b`): completes over the buffers, other buffers most recent first and the current one last, so an empty `RET` goes back to the previous buffer. A name no buffer has creates an empty buffer.
- New command `switch-to-buffer-other-window` (`C-x 4 b`): the same, in the next window, splitting the frame below when it has one window, and selects that window.
- New command `kill-buffer` (`C-x k`): completes over the buffers, the current one first. A modified buffer visiting a file asks `Buffer modified; kill anyway? ` first, completing over `no` and `yes`. Windows showing the killed buffer show the most recent other buffer, or a new `*scratch*` when none is left.
- A buffer remembers the point of the last window that stopped showing it, and a window that shows it again starts there.
- Buffers whose names start with a space (the minibuffer's) are left out of the lists.
- Buffer names are unique: a new buffer whose name is taken gets `<2>`, `<3>` and so on.

## Capabilities

### New Capabilities
- `buffer-switching`: switching the selected or next window to a buffer by name, killing buffers, the buffer order and remembered points.

### Modified Capabilities

None.

## Impact

- `crates/aiditor-core`: `buffer.rs` (recency order, unique names, each buffer's remembered point), `window.rs` (windows start at and record the buffer's point), `lib.rs` (the three commands and their completions), `keymap.toml`, harness tests.
- `crates/aiditor`: unchanged; the new bindings reach the core through the existing key path.
- `docs/editor.md`: the decisions taken here, under Commands and keymaps.
- No new dependencies.
