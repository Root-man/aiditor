# Proposal

## Why

Since #81 a buffer with unsaved edits whose file changes on disk keeps its text, and its save is refused with nothing else offered: the user has no way to tell the editor which version wins short of `M-x revert-buffer` or editing the file elsewhere. Issue [#83](https://github.com/Root-man/aiditor/issues/83) adds the conflict flow `docs/editor.md` describes under Core data model: the buffer is marked conflicted, and focusing or saving it asks keep mine, take disk or compare.

## What Changes

- A watcher check that finds a modified buffer's file changed (present, hash other than the buffer's) marks the buffer conflicted; the mode line shows ` [conflict]` after the name. A file that goes back to the buffer's hash, is deleted, or is taken by a revert or a save ends the conflict.
- Focusing a conflicted buffer (it becomes the selected window's buffer: `C-x b`, `C-x o`, a click, a workspace switch, `C-x C-f`) asks `<path> changed on disk: ` with `Compare`, `Take disk`, `Keep mine`, once per disk version; never over an open minibuffer, and not when the conflict arrives while the buffer is already focused.
- `C-x C-s` on a conflicted buffer asks the same question, every time.
  - **Keep mine**: the buffer takes the disk version as its base, so the save overwrites it; from `C-x C-s` the save goes ahead.
  - **Take disk**: the buffer reverts to the file as `revert-buffer` does; the unsaved text is one `C-/` away.
  - **Compare**: `diff-buffer-with-file` (#82) shows the diff in `*Diff*`; the conflict stays.
- `M-x resolve-conflict` asks the question for the current buffer at any time.
- A save refused as a change on disk checks the file, so a change the watcher missed becomes a conflict and the next `C-x C-s` asks.
- `docs/editor.md` records it.
- Not included: asking in `C-x C-c`'s and workspace kill's save prompts (a conflicted buffer's save there is refused, cancelling the quit, as before); a three-way merge.

## Capabilities

### New Capabilities
- `file-conflicts`: the conflicted state, when the question is asked, and what each answer does.

### Modified Capabilities
- `file-watching`: a modified buffer whose file changes becomes conflicted instead of only keeping its text.
- `display-model`: the mode line shows ` [conflict]`.

## Impact

- `aiditor-core`: `Buffer` gains the conflict's disk hash and whether focus already asked; `watching.rs` marks and clears conflicts and asks on focus; `visiting.rs` gets the question and its answers, `save-buffer` routes through it, and a refused save checks the file; `display.rs` mode line; `builtins.rs` registers `resolve-conflict`; `lib.rs` tracks the focused buffer.
- `docs/editor.md`: a "Built in #83" note.
