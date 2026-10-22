# Proposal

## Why

`C-s` was decided to run swiper ([#72](https://github.com/Root-man/aiditor/issues/72)), but nothing searches inside a buffer yet. Issue [#75](https://github.com/Root-man/aiditor/issues/75) asks for swiper itself: the buffer's matching lines listed in the minibuffer with a preview, `RET` jumping to the selected one. It replicates ivy's swiper (`swiper.el`, `ivy.el`).

## What Changes

- New `swiper` command bound to `C-s` that prompts `Swiper: `. It lists every line of the current buffer in buffer order, each line shown after its line number, which is not matched.
- Swiper's matching (`ivy--regex-plus`): words separated by spaces must appear in order on the line. Each word is a regexp in the Rust `regex` syntax, matched literally when it isn't valid; `\ ` is a literal space. Text after `!` excludes the lines its words match. Matching is case-insensitive while the input has no upper-case letter.
- The list opens with the line at point selected. After an edit to the input it keeps the selected line if that line still matches, else selects the next line that does, else the last match.
- Preview: the window shows the selected line with point at the end of the first match, recentering only when the line is out of view. With empty input on the starting line, point stays where it was.
- `RET` moves point to the end of the match, recentering if needed, and saves the mark where the search started (`Mark saved where search started`) when point moved and no region is active. `C-g` puts the window back as it was. When nothing matches, `RET` says `Match required` and keeps the minibuffer open.
- `C-s` inside swiper selects the next match, or takes the previous search from history when the input is empty. History keeps the typed searches, including those of a search ended with `C-g`.
- Minibuffer groundwork that swiper needs:
  - A command can filter its own candidates in place of the fuzzy matcher. Its matches keep their given order and follow the selection rule above.
  - A command can preselect a candidate.
  - A preview can name a position in the window's own buffer.
  - Its minibuffer can use a keymap named after the command.
- Deferred: highlighting the selected line and the matches in the window. Highlighting needs text properties that can be removed or scoped to a window, which the core doesn't have yet.
- Not included: swiper's `M-q`, `C-l`, avy, multiple cursors, face matching, occur, and visual-line candidates.

## Capabilities

### New Capabilities
- `line-search`: the swiper command: its candidates, matching, selection, preview, jump, mark and history.

### Modified Capabilities
- `minibuffer`: adds candidates filtered and ordered by the command, a preselected candidate, previewing a position in the original buffer, a keymap per command, and a match required by filtered prompts.

## Impact

- `crates/aiditor-core`:
  - `command.rs`: `ArgSpec` gains `filter` and `preselect`, and `Preview` gains `Point`.
  - `minibuffer.rs`: filtered completion, preselect, point preview, the command's minor mode, the typed text kept after exit, and history on abort.
  - New `swiper.rs`.
  - `builtins.rs` and `keymap.toml` register the commands and bind the keys.
- `regex` becomes a direct dependency of `aiditor-core`. It is already in `Cargo.lock` as a transitive dependency.
- `docs/editor.md`: the Minibuffer and Editing commands notes.
