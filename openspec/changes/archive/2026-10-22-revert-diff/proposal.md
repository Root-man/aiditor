# Proposal

## Why

Auto-revert, "Take disk" in the conflict flow and the disk diff view (M1) all need to replace a buffer's text with the file's without losing point, mark and markers. Replacing the whole text would move every one of them to the start. Issue [#79](https://github.com/Root-man/aiditor/issues/79) asks for a revert that applies a diff instead, computed off the core thread, as one undo group. That settles the open question of how a revert keeps point, mark and markers.

## What Changes

- The file service reverts on request. It reads the file, decodes it as a visit does, and diffs the buffer's text against it line by line with `similar`. It replies with the edits that turn the buffer's text into the file's.
- The core applies those edits as one undo group, so `C-/` brings the old text back. Markers move with the edits: point, mark and every other marker stay with their text. Afterwards the buffer is unmodified and keeps the file's hash, line ending and BOM.
- A reply for a buffer edited since the request is dropped, since its edits are relative to text that is gone.
- New command `revert-buffer` (`M-x`), the first user of the mechanism:
  - It asks `Revert buffer from file <path>? ` (`Yes`, `No`) only when the buffer is modified, like Emacs's `revert-buffer-quick`.
  - A buffer visiting no file gets `Buffer does not seem to be associated with any file`.
  - A missing file gets `File <path> no longer exists`.
  - A binary or non-UTF-8 file gets `<path>: <reason>`, and the buffer stays untouched.
- `docs/editor.md` records the decision.
- Not included:
  - auto-revert and the watcher (#80 and later);
  - char-level diffs inside changed lines;
  - a key binding (Emacs's `C-x x g`).

## Capabilities

### New Capabilities
- `file-reverting`: the revert diff, how it is applied (one undo group, markers kept, buffer left unmodified), stale replies, and the `revert-buffer` command.

### Modified Capabilities

## Impact

- `aiditor-core`:
  - a `similar` dependency;
  - `buffer::diff` next to `encode` and `decode`;
  - `Buffer::replace` to apply edits;
  - `FileOp::Revert` and `Msg::Reverted` in `lib.rs`, the `Revert` struct beside `buffer::revert`;
  - the `revert-buffer` command and its reply handler in `visiting.rs`;
  - command registration in `builtins.rs`;
  - the harness's answer to `FileOp::Revert`.
- `aiditor-services`: `files::run` answers `FileOp::Revert`. The app and the headless binary pick it up with no change.
- `docs/editor.md`: the Open questions entry and the "still to be decided" line under File watching.
