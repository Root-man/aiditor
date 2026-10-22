# Proposal

## Why

The conflict flow's "Compare" (#83) and recovered buffers both need to show how a buffer's text differs from its file on disk. Recovery's "compare it with the disk" has been "Not yet (M1)" since #51. Issue [#82](https://github.com/Root-man/aiditor/issues/82) asks for a read-only buffer holding a unified diff of the buffer against the disk, built on the revert diff from #79.

## What Changes

- New command `diff-buffer-with-file` (`M-x`, Emacs's name) for the current buffer. The file service reads and decodes the file as a revert does, and diffs the disk text against the buffer's text into a unified diff (`--- <path>`, `+++ <buffer name>`, three lines of context).
- The diff goes into a read-only `*Diff*` buffer, one per workspace, refilled on each run and without undo. It is shown in another window without selecting it, splitting a lone window, as Emacs's `display-buffer` does.
- With no differences the command says `No differences` and opens nothing.
- A buffer visiting no file, a missing file and a binary file give `revert-buffer`'s messages.
- A recovered buffer is compared by recovering it (`Yes`) and running `M-x diff-buffer-with-file`. The recovery prompt itself is unchanged.
- `docs/editor.md` records it.
- Not included:
  - colors for added and removed lines;
  - jumping from a hunk to the source line (`diff-goto-source`);
  - a prompt for the buffer to compare, or a key binding;
  - line-ending and BOM differences, which the decoded texts don't show.

## Capabilities

### New Capabilities
- `file-diffing`: the `diff-buffer-with-file` command, the `*Diff*` buffer it fills and shows, and its messages.

### Modified Capabilities

## Impact

- `aiditor-core`:
  - `buffer::unified_diff`, sharing the read and decode step with `buffer::revert`;
  - `Buffer::set_text`, replacing the text past read-only without undo;
  - `FileOp::Diff` and `Msg::Diffed` in `lib.rs`;
  - the command and its reply handler in `visiting.rs`, registered in `builtins.rs`;
  - the harness's answer to `FileOp::Diff`.
- `aiditor-services`: `files::run` answers `FileOp::Diff`; the app and headless binary need no change.
- `docs/editor.md`: the recovery "Not yet" and a note under Reverting.
