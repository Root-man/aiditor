# Proposal

## Why

A file can only be visited by naming it on the command line. Issue #41 adds Emacs's `find-file` (`C-x C-f`): a prompt that starts at the current buffer's directory, completes over that directory's entries and visits the chosen file, or makes a new buffer for a file that does not exist yet.

## What Changes

- New command `find-file` (`C-x C-f`) with the prompt `Find file: `, its text starting as the current buffer's directory (the file's directory, or the working directory for a buffer without a file).
- The prompt completes over the entries of the directory typed so far, fuzzy-matched against the text after the last `/`; subdirectories end in `/`. Directory listings come from the file service on a background thread, and the list fills when the listing arrives.
- `TAB` puts the selected entry's full path in the text; `RET` on a selected directory enters it the same way instead of exiting.
- `DEL` at the end of a file prompt deletes a whole directory when the text ends in `/` or names a listed directory (Vertico's `vertico-directory-delete-char`), one char otherwise.
- New command `minibuffer-exit-input` (`M-RET` in completing minibuffers) returns the text as typed even when candidates match, so a new file whose name fuzzy-matches an existing one can be made.
- An existing file is read on a background thread and visited in a new buffer; a missing one gets an empty buffer visiting it, written on the first save. A file already visited switches to its buffer instead of making a second one.
- A binary or non-UTF-8 file is refused with the reason in the echo area, as decided in #45; an unreadable file or a directory shows the error.

## Capabilities

### New Capabilities

None.

### Modified Capabilities
- `file-visiting`: visiting files from the minibuffer with `find-file`.
- `minibuffer`: returning the typed text with `M-RET` while candidates match.

## Impact

- `crates/aiditor-core`: `lib.rs` (the command, file-name completion, new `Effect`s and `Msg`s), `command.rs` (a spec flag for file-name prompts), `keymap.toml`, harness and tests.
- `crates/aiditor-services`: `files.rs` lists a directory.
- `crates/aiditor`: `main.rs` runs the new effects on Tokio's blocking pool.
- `docs/editor.md`: the decisions, under Commands and keymaps.
- No new dependencies.
