# Proposal

## Why

Only a global `buffer-list` order exists, so there is no Emacs-style `C-x <left>`/`C-x <right>` ([#341](https://github.com/Root-man/aiditor/issues/341)), and a window showing a killed buffer falls back to the workspace's most recent buffer rather than the one it showed before. That fallback is the cause of a reported bug: open `*Workspaces*` from one window of a split, pick another workspace, switch back, and both windows show the same buffer, because the list's window took the most recent buffer (often the other window's) when the list was killed.

## What Changes

- Each window keeps the buffers it showed before, most recent last, and the buffers `previous-buffer` left, for `next-buffer`. Splitting a window copies its history; a workspace's saved windows keep theirs.
- New `previous-buffer` (`C-x <left>`) and `next-buffer` (`C-x <right>`) cycle the selected window through its history, as Emacs's per-window previous buffers do. A window with no history left falls back to the workspace's other listed buffers in `buffer-list` order (most recent first for `previous-buffer`, least recent first for `next-buffer`).
- Killing a buffer drops it from every window's history, in every workspace, and a window that showed it shows its own previous live buffer, falling back to today's rule (the workspace's most recent other listed buffer, `*Messages*` or a new scratch) only when it has none.

## Capabilities

### New Capabilities

### Modified Capabilities
- `buffer-switching`: new requirement for cycling a window's buffers; killing a buffer replaces it in each window with that window's previous buffer first.
- `workspaces`: a killed buffer's replacement in another workspace's windows is that window's previous buffer first.

## Impact

- `crates/aiditor-core/src/window.rs` (history on `Window`, recorded by `show`, copied by `split`), `workspace.rs` (`Windows::replace_buffer`), `visiting.rs` (`kill_buffer`, the two commands), `builtins.rs`, `keymap.toml`.
- `docs/editor.md` Commands and keymaps, Buffers.
- No API or dependency changes.
