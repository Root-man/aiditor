# Proposal

## Why

The minibuffer reads a string, but a prompt can't offer choices, so `M-x`, `find-file` and buffer switching have nothing to pick from. Issue #39 adds the vertical fuzzy completion list they all share.

## What Changes

- A command can declare candidates for its prompts. When it does, its minibuffer shows a vertical list of the candidates that fuzzy-match the text, best first, all of them in the given order while the text is empty, and narrows it as the user types.
- Up to 10 candidates show at a time, below the prompt line; the selected one is highlighted and kept in view. The selection goes back to the first match whenever the text changes.
- `C-n` and `C-p` (`minibuffer-next-completion`, `minibuffer-previous-completion`) move the selection, repeated by the prefix argument and stopping at the ends. `TAB` (`minibuffer-complete`) replaces the text with the selected candidate. `RET` returns the selected candidate, or the typed text when nothing matches.
- These keys live in a `minibuffer-completion` keymap that only a completing minibuffer uses, so plain prompts keep `C-n`, `C-p` and `TAB` as before.
- Matching runs on `nucleo`'s own worker threads; the core polls it after each message and on each frame, and re-polls on a short timer while matching is still running, so no Tokio service is needed.
- Not yet: highlighting the matched chars, a match count, cycling past the ends, returning the typed text while something matches, different candidates per prompt.

## Capabilities

### New Capabilities

### Modified Capabilities
- `minibuffer`: a prompt can complete over candidates with a fuzzy-filtered list, moved through with `C-n`/`C-p`, completed with `TAB` and picked with `RET`.

## Impact

`aiditor-core` (`command.rs` `ArgSpec`, `lib.rs` minibuffer and frame display, `display.rs`, `keymap.toml`), the frontend's echo area in `aiditor/src/main.rs`, and the minibuffer bullet in `docs/editor.md`. New dependency in `aiditor-core`: `nucleo`, the matcher the crate shortlist already chose; it brings no GPUI or Tokio.
