# Design

## Context

The shown diff holds the old side's text and the hunks (old and new line ranges) against the new side's buffer: under `Changes` old is the index and new the file's buffer, under `Staged` old is `HEAD` and new the read-only index buffer. `FileOp::GitUpdate` already runs staging steps in a repository and refreshes the list on success (#510).

## Goals / Non-Goals

**Goals:** the issue's acceptance criteria, with one path for stage and unstage.

**Non-Goals:** CRLF or BOM files' patches (see risks), choosing removed rows one by one (the region is on the new side, so a touched change's removed rows go whole), the side-by-side view.

## Decisions

- **Edits, then a patch against the index.** Each chosen change is a line-range edit of a base text. Staging: base is the index (the `Changes` diff's old text), each change's old lines replaced by its chosen new lines. Unstaging and discarding: base is the new side (the index buffer, or the file's buffer), each change's new lines replaced by its old lines followed by its unchosen new lines. A patch is the unified diff (`similar`, 3 lines of context, the no-newline marker) of base against the edited text, with a `diff --git a/<file> b/<file>` header. So unstaging needs no `--reverse`: the patch is built against the index directly, the same way as staging. Alternative considered: writing the blob with `hash-object` and `update-index`; rejected because `git apply`'s context check catches an index changed since the diff.
- **Joining a line without a newline.** The old side's last line may lack its newline; when more text follows it in an edit, a newline is added, since only a text's last line can lack one.
- **`GitStep::Apply(patch)`** in `FileOp::GitUpdate`, run as `git apply --cached -` with the patch on stdin; it takes no paths, so the step filter keeps it with an empty path list. `GitStep` loses `Copy`.
- **Same commands, two windows.** `diff-stage`, `diff-unstage` and `diff-discard` look at whether the diff window is selected: if so they act on the change; otherwise on the list row, as now. `C-c s/u/k` go in the `diff-view` keymap, so they work from either window.
- **Whole files.** A file whose state in the diff's group is added, deleted, untracked, renamed or copied has no index entry, or one under another path, on a side a patch would need, so `C-c s`/`C-c u` stage or unstage it whole with the list's steps. Discarding in a `Changes` diff edits the buffer for every state, an untracked file's buffer emptying.
- **Discard in the buffer.** The edits are turned into char ranges and applied with `Buffer::replace` between two `undo_boundary` calls, as a revert is. A read-only buffer refuses with `Buffer is read-only`. The re-diff timer then redraws the diff.
- **Stale hunks.** Acting while a re-diff is awaited or pending refuses with `The diff is being worked out`, so a patch is never built from hunks of an older text.
- **Point kept.** `on_git_status` records the diff window's row and (line, col) before it drops the shown diff; when the same row's hunks first arrive, point goes there (clamped) instead of to the first change.

## Risks / Trade-offs

- Patches are built from decoded LF text, so a CRLF or BOM file's patch fails to apply and git's message is echoed. Left as a `shortcut:`.
- A path with `"`, `\` or a control char would need git's quoting in the header; such a path fails with git's message.
