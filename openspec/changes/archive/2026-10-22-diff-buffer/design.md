# Design

## Context

- #79 added `buffer::revert(path, old, read)`: it turns the file service's read into decoded text, or the `File <path> no longer exists` / `<path>: <reason>` errors, then diffs it with `similar`. `files::run` and the harness each read the file and call it for `FileOp::Revert`.
- `Buffer::append` writes past read-only without undo and keeps the tick, as `*Messages*` needs.
- `switch-to-buffer-other-window` splits a lone window and selects the other; windows other than the selected one live in `Core::others`, shown with `Window::show`. While the minibuffer is open the selected window is the minibuffer's, outside the tree.

## Goals / Non-Goals

**Goals:**
- reuse revert's read/decode and `similar`; no new dependency;
- give #83's "Compare" one call to make.

**Non-Goals:**
- faces, `diff-goto-source`, char-level hunks;
- refreshing `*Diff*` when the buffer or file changes.

## Decisions

- **Pure `buffer::unified_diff(path, name, text, read) -> Result<String, String>`** beside `revert`. Both start with a shared `disk_text(path, read)` that yields the decoded text (and CRLF, BOM, hash for revert), so the error messages stay one. It renders `similar::TextDiff::from_lines(disk, buffer).unified_diff().context_radius(3).header(path, name)`; `similar` writes no header when there are no hunks, so an empty string means no differences. Old = disk, new = buffer, as Emacs's `diff-buffer-with-file` runs `diff file buffer`.
- **`FileOp::Diff { buffer, name, path, text }` / `Msg::Diffed { buffer, result }`.** The buffer's name travels in the op, so the service renders the whole text, header included, and the core only inserts it.
- **`Buffer::set_text(text)`**: delete all and insert past read-only, without undo, keeping `modified`, like `append`. Refilling `*Diff*` this way leaves it unmodified and with nothing to undo. Alternative: kill and recreate `*Diff*`; rejected because windows showing it would switch away first.
- **Display (`Core::display_buffer`)**: if a window (selected or other) already shows the buffer, nothing more; else split the selected window when it is the only leaf, and show the buffer in the next leaf after the selected one in tree order (as `other-window` picks), leaving the selection. The selected window is unchanged, so no jump is pushed. shortcut: skipped while the minibuffer is open (an answer arriving mid-prompt); the buffer is still filled for `C-x b`.
- **Where**: the command and `on_diffed` live in `visiting.rs` next to `revert_buffer` / `on_reverted`. A result whose buffer is gone or not in the current workspace (`Buffers::is_here`) is dropped. Errors echo as revert's do.
- **`*Diff*` per workspace**: found as the current workspace's buffer in `diff-mode`, not by name, since a second workspace's is `*Diff*<2>`; made with `create`, owned by the current workspace; `read_only = true`, `major_mode = "diff-mode"` (no keymap yet; it names one for later).

## Risks / Trade-offs

- [A huge diff string is inserted on the core thread] → rope insert is linear and fast; the diffing itself is on the service.
- [`*Diff*` goes stale as the buffer is edited] → as in Emacs; run the command again.
- [CRLF/BOM-only differences show as `No differences`] → the save would still write them; documented.
