## Why

Issue #45 is the open question "File handling: encodings, CRLF line endings, final newline, very long lines, binary files, keeping file permissions on save", plus atomic saves versus symlinks and hard links and whether long lines wrap. #46 (file service) is blocked on it, and since #11 a line wider than the window puts the cursor off screen.

## What Changes

- Record the decision in `docs/editor.md` under Core data model, with Emacs's defaults:
  - UTF-8 only, the BOM kept; other encodings in M5 (new task #155 under new epic #154);
  - the file's line ending detected, LF in the buffer, written back on save, `(DOS)` in the mode line;
  - a final newline added on save (`require-final-newline` = t);
  - binary or non-UTF-8 files refused with a message; a command-line file opens as an empty read-only buffer;
  - atomic saves (temp file and rename on the symlink target, mode copied), in place for hard-linked files or when the rename is not allowed;
  - long lines truncated with `auto-hscroll-mode` = `current-line` in M0 (new task #153), Emacs-like soft wrap in M5 (#112).
- Remove the question from Open questions, reword the watcher's atomic-save bullet to the part still open, and narrow the text-layout question to the session buffer.
- `docs/roadmap.yaml` and GitHub: #46 gets the rules as acceptance criteria, #112 gets the wrap rules, and #153, #154 and #155 are added.

No code changes: the behavior is built in #46, #153 and #155.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. This change only records a decision; `skip_specs` is set and the specs arrive with #46, #153 and #155.

## Impact

- `docs/editor.md`, `docs/roadmap.yaml`, issues #46, #112, #153, #154, #155.
