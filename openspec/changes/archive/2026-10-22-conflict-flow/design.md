# Design

## Context

- `on_checked` (`watching.rs`) gets each checked file's hash: gone sets `deleted`, a different hash reverts a clean buffer, a modified buffer is left alone (#81).
- `revert_buffer` asks only for a modified buffer and pushes `FileOp::Revert`; `on_reverted` applies the diff as one undo group, sets `disk` and marks the buffer saved. `diff_buffer_with_file` pushes `FileOp::Diff` (#82).
- `Ctx::save` sends `FileOp::Save` with `buffer.disk`; the file service refuses when the file's hash differs. `save_some` (quit, workspace kill) calls `Ctx::save` directly; `save-buffer` is the only interactive single save.
- `read_string(command, args, prompt, choices)` reopens `command` with the answer appended to `args.strings`.

## Goals / Non-Goals

**Goals:** the three answers in `docs/editor.md`, reusing revert, diff and save as they are.

**Non-Goals:** asking inside `save_some`'s prompts; three-way merge; asking when the OS window regains focus.

## Decisions

- **State on `Buffer`: `conflict: Option<u64>`**, the hash of the disk version the unsaved edits conflict with, plus `conflict_asked: bool`. Keeping the hash lets Keep mine make that version the save's base (`disk = conflict`) without a new `FileOp::Save` flag to skip the check; if the file changed again since, the save is still refused, which is the point of the check. Alternative: a bool plus a forced save; rejected as it could overwrite a version nobody looked at.
- **Marking in `on_checked`**: present and equal to `disk` clears it; present, different and modified sets it (resetting `conflict_asked` when the hash is new); gone clears it (the deleted state's save writes back without loss). `on_reverted` and a successful `on_saved` clear it, since the buffer then holds or wrote the disk's version.
- **Focus = the selected window's buffer changed since the last handled message**, tracked as `Core::focused`, checked at the end of `Core::handle` (after timers) in `Ctx::ask_on_focus`. While the minibuffer is open `focused` is not updated, so `C-x b`'s final switch counts once it closes and previews don't. One check covers every way of switching (keys, clicks, workspaces, find-file) without touching each.
- **Once per disk version on focus, every time on save.** Re-asking on each focus would nag while merging by hand after Compare (flipping between `*Diff*` and the buffer); the mode line keeps showing the conflict and the save always asks. A conflict that arrives while the buffer is focused waits for the next focus or save rather than opening a prompt mid-typing, where typed keys would land in the minibuffer.
- **One handler, two commands.** `Ctx::resolve_conflict(command, args)` serves `resolve-conflict` and `save-buffer`; Keep mine saves only when the command is `save-buffer`. Choices are ordered safe to destructive, `Compare`, `Take disk`, `Keep mine`, so `RET` right away loses nothing. Take disk calls `revert_buffer` with `Yes`; Compare calls `diff_buffer_with_file`. An answer arriving after the conflict ended: `save-buffer` just saves, `resolve-conflict` does nothing. An unknown typed answer does nothing.
- **A refused save checks the file.** `on_saved`'s error path pushes `FileOp::Check` for the buffer's file, so the existing check path marks it conflicted (or deleted). Any failed save checks; checking a path is harmless.

## Risks / Trade-offs

- [Keep mine on a buffer undone back to unmodified saves nothing (`No changes need to be saved`)] → rare; the buffer is clean and the next change auto-reverts it.
- [C-x C-c with a conflicted buffer answered `Yes` is refused and cancels the quit] → as before #83; resolve with `C-x C-s` or `M-x resolve-conflict` first.
